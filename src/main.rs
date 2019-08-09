use futures::{future, Future, Stream};
use hyper::http::header::CONTENT_TYPE;
use hyper::http::Result;
use hyper::service::service_fn;
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use serde::{Deserialize, Serialize};
use std::convert::Into;
use std::fmt;
use std::str::FromStr;

type FutureResponse<A> = Box<dyn Future<Item = Response<A>, Error = hyper::Error> + Send>;

#[derive(Debug, Deserialize, Serialize)]
struct Pet {
    id: u64,
    name: String,
    tag: Option<String>,
}

#[derive(Serialize)]
struct Pets {
    items: Option<Vec<Pet>>,
}

#[derive(Serialize)]
struct Error {
    code: u32,
    message: String,
}

// Helpers to parse incomming requests
fn get_method<A>(req: &Request<A>) -> &Method {
    req.method()
}

fn get_path_segments<A>(req: &Request<A>) -> Vec<&str> {
    req.uri().path().trim_matches('/').split('/').collect()
}

fn get_query_parameter<A, B: FromStr>(req: &Request<A>, parameter_name: &str) -> Option<B> {
    req.uri()
        .query()
        .unwrap_or("")
        .split('&')
        .map(|name_value| name_value.split('=').collect())
        .flat_map(|name_value: Vec<&str>| match name_value.as_slice() {
            [name, value] => vec![(name.to_string(), value.to_string())],
            [name] => vec![(name.to_string(), "true".to_string())],
            _ => vec![],
        })
        .find(|(name, _)| name == parameter_name)
        .and_then(|(_, value)| value.parse::<B>().ok())
}

// Database interations aren't part of this example
fn doa_find_pets(limit: u32, offset: u32) -> Pets {
    let items = (offset..(offset + limit))
        .map(|i| doa_find_pet_by_id(i.into()))
        .collect();

    Pets { items: Some(items) }
}

fn doa_find_pet_by_id(pet_id: u64) -> Pet {
    Pet {
        id: pet_id,
        name: "john".to_string(),
        tag: Some("doe".to_string()),
    }
}

fn doa_create_pet(pet: Pet) {
    println!("Creating {:?}", pet);
}

// Handle requests
fn list_pets(limit: u32, offset: u32) -> Result<Response<Body>> {
    let pets = &doa_find_pets(limit, offset);
    let json = serde_json::to_string(pets).unwrap();
    let x_next = format!("/pets?limit={}&offset={}", limit, limit + offset);

    Response::builder()
        .status(StatusCode::OK)
        .header("x-next", x_next)
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(json))
}

fn create_pets(pet: Pet) -> Result<Response<Body>> {
    doa_create_pet(pet);

    Response::builder()
        .status(StatusCode::CREATED)
        .body(Body::empty())
}

fn show_pet_by_id(pet_id: u64) -> Result<Response<Body>> {
    let pet = &doa_find_pet_by_id(pet_id);
    let json = serde_json::to_string(pet).unwrap();

    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(json))
}

fn not_found() -> Result<Response<Body>> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::empty())
}

// Parse requests
fn router(req: Request<Body>) -> FutureResponse<Body> {
    match (get_method(&req), get_path_segments(&req).as_slice()) {
        (&Method::GET, ["pets"]) => {
            let limit = get_query_parameter(&req, "limit").unwrap_or(25);
            let offset = get_query_parameter(&req, "offset").unwrap_or(0);
            let res = list_pets(limit, offset).unwrap();

            Box::new(future::ok(res))
        }
        (&Method::POST, ["pets"]) => {
            let res = req
                .into_body()
                .concat2()
                .map(|body| serde_json::from_slice::<Pet>(&body).unwrap())
                .map(|pet| create_pets(pet).unwrap());

            Box::new(res)
        }
        // Swagger specification has pet_id as String in parameter, but u64 in schema
        // Using u64 accross the board for consistency
        (&Method::GET, ["pets", pet_id]) if pet_id.parse::<u64>().is_ok() => {
            let typed_pet_id = pet_id.parse::<u64>().unwrap();
            let res = show_pet_by_id(typed_pet_id).unwrap();
            Box::new(future::ok(res))
        }
        _ => {
            let res = not_found().unwrap();
            Box::new(future::ok(res))
        }
    }
}

// Start server
fn main() {
    let addr = ([127, 0, 0, 1], 3000).into();

    let server = Server::bind(&addr)
        .serve(|| service_fn(router))
        .map_err(|e| eprintln!("server error: {}", e));

    println!("Listening on http://{}", addr);

    hyper::rt::run(server);
}
