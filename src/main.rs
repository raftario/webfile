use hyper::{body, service, Body, Method, Request, Response, Server, StatusCode};
use lazy_static::lazy_static;
use std::{convert::Infallible, env, net::SocketAddr, path::PathBuf, process};
use tokio::{fs, signal};

static HTML: &str = include_str!("../resources/index.html");
lazy_static! {
    static ref FILENAME: PathBuf = PathBuf::from(env::args().skip(1).next().unwrap_or_else(|| {
        eprintln!("Missing filename argument");
        process::exit(1);
    }));
}

async fn webfile(req: Request<Body>) -> Result<Response<Body>, Infallible> {
    let response = match (req.method(), req.uri().path()) {
        (&Method::GET, "/") => {
            let contents = fs::read_to_string(&*FILENAME)
                .await
                .map(|s| {
                    s.replace("\n", "\\n")
                        .replace("\r", "\\r")
                        .replace("\"", "\\\"")
                })
                .unwrap_or(String::new());
            Response::new(Body::from(
                HTML.replace(
                    "{{ title }}",
                    FILENAME.file_name().unwrap().to_str().unwrap(),
                )
                .replace("{{ contents }}", &contents),
            ))
        }
        (&Method::PUT, "/") => {
            let body = match body::to_bytes(req.into_body()).await {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("Invalid request body: {}", e);
                    return Ok(Response::builder()
                        .status(StatusCode::BAD_REQUEST)
                        .body(Body::from("Bad Request"))
                        .unwrap());
                }
            };

            match fs::write(&*FILENAME, body).await {
                Ok(_) => Response::builder()
                    .status(StatusCode::NO_CONTENT)
                    .body(Body::empty())
                    .unwrap(),
                Err(e) => {
                    eprintln!("IO error: {}", e);
                    Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Body::from("Internal Server Error"))
                        .unwrap()
                }
            }
        }
        _ => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("Not Found"))
            .unwrap(),
    };
    Ok(response)
}

async fn shutdown() {
    signal::ctrl_c()
        .await
        .expect("Failed to install CTRL+C signal handler");
}

#[tokio::main]
async fn main() {
    {
        let _filename = &*FILENAME;
    }
    let port: u16 = env::args()
        .skip(2)
        .next()
        .map(|p| {
            p.parse().unwrap_or_else(|_| {
                eprintln!("Invalid port argument");
                process::exit(1);
            })
        })
        .unwrap_or(3000);
    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    let make_svc = service::make_service_fn(|_conn| {
        async { Ok::<_, Infallible>(service::service_fn(webfile)) }
    });
    let server = Server::bind(&addr)
        .serve(make_svc)
        .with_graceful_shutdown(shutdown());
    println!("Listening on port {}", port);

    server.await.unwrap_or_else(|e| {
        eprintln!("Server error: {}", e);
        process::exit(1);
    });
}
