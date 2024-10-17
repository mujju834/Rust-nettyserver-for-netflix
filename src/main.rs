use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use dotenv::dotenv;
use reqwest::Client;
use std::env;
use std::io::Error;

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Load environment variables from .env file
    dotenv().ok();

    // Load environment variables into Rust variables
    let api_gateway_url = env::var("API_GATEWAY_URL")
        .expect("API_GATEWAY_URL not found in .env");
    let server_port = env::var("SERVER_PORT")
        .expect("SERVER_PORT not found in .env");

    println!("API_GATEWAY_URL: {}", api_gateway_url);
    println!("SERVER_PORT: {}", server_port);

    // Start the HTTP server
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(Client::new())) // Reqwest client for forwarding requests
            .app_data(web::Data::new(api_gateway_url.clone())) // Store the API gateway URL
            .service(web::resource("/{tail:.*}").to(proxy_handler)) // Route all requests
    })
    .bind(format!("0.0.0.0:{}", server_port))?
    .run()
    .await
}

// Proxy handler function to forward requests or return custom responses
async fn proxy_handler(
    client: web::Data<Client>,
    api_gateway_url: web::Data<String>,
    req: HttpRequest,
    mut body: web::Payload,
) -> impl Responder {
    let path = req.match_info().query("tail");
    let url = format!("{}/{}", api_gateway_url.get_ref(), path);

    println!("Received {} request for {}", req.method(), path);

    // Handle root endpoint
    if path.is_empty() {
        return HttpResponse::Ok()
            .content_type("text/plain")
            .body("Netty server deployed by Mujahid in Rust");
    }

    // Handle CORS preflight requests
    if req.method() == actix_web::http::Method::OPTIONS {
        return HttpResponse::Ok()
            .append_header(("Access-Control-Allow-Origin", "*"))
            .append_header((
                "Access-Control-Allow-Methods",
                "POST, GET, OPTIONS, PUT, DELETE",
            ))
            .append_header((
                "Access-Control-Allow-Headers",
                "Content-Type, Authorization, Range",
            ))
            .finish();
    }

    // Forward request to API Gateway
    let forwarded_req = client
        .request(req.method().clone(), &url)
        .headers(req.headers().clone().into()) // Convert headers to reqwest's HeaderMap
        .send()
        .await;

    match forwarded_req {
        Ok(resp) => {
            let mut response = HttpResponse::build(resp.status());

            // Set CORS headers in the response
            response
                .append_header(("Access-Control-Allow-Origin", "*"))
                .append_header((
                    "Access-Control-Allow-Methods",
                    "POST, GET, OPTIONS, PUT, DELETE",
                ))
                .append_header((
                    "Access-Control-Allow-Headers",
                    "Content-Type, Authorization, Range",
                ));

            // Copy all headers from the forwarded response
            for (key, value) in resp.headers() {
                response.append_header((key.clone(), value.clone()));
            }

            // Stream the response body
            let body = resp.bytes().await.unwrap_or_default();
            response.body(body)
        }
        Err(e) => {
            eprintln!("Error forwarding request: {}", e);
            HttpResponse::ServiceUnavailable().body("Service unavailable")
        }
    }
}
