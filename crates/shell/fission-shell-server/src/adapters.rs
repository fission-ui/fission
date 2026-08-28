use crate::{ServerRenderer, ServerRequest, ServerResponse};
use std::sync::Arc;

/// Adapter for Hyper request and response types.
pub mod hyper_adapter {
    use super::*;
    use hyper::{Body, Request, Response, StatusCode};
    use std::convert::Infallible;

    /// Converts a Hyper request, renders it, and converts the result back to Hyper.
    pub async fn handle(
        renderer: Arc<ServerRenderer>,
        request: Request<Body>,
    ) -> Result<Response<Body>, Infallible> {
        let method = request.method().as_str().to_string();
        let path = request.uri().path().to_string();
        let query = parse_query(request.uri().query().unwrap_or(""));
        let headers = request
            .headers()
            .iter()
            .filter_map(|(name, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|value| (name.as_str().to_ascii_lowercase(), value.to_string()))
            })
            .collect();
        let body = hyper::body::to_bytes(request.into_body())
            .await
            .map(|bytes| bytes.to_vec())
            .unwrap_or_default();
        let response = renderer
            .handle(ServerRequest {
                method,
                path,
                query,
                headers,
                body,
            })
            .unwrap_or_else(|error| {
                ServerResponse::text(500, "text/plain; charset=utf-8", error.to_string())
            });
        Ok(to_hyper_response(response))
    }

    fn to_hyper_response(response: ServerResponse) -> Response<Body> {
        let mut builder = Response::builder().status(
            StatusCode::from_u16(response.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        );
        for (name, value) in &response.headers {
            builder = builder.header(name.as_str(), value.as_str());
        }
        builder.body(Body::from(response.body)).unwrap()
    }
}

#[cfg(feature = "axum-adapter")]
pub mod axum_adapter {
    use super::*;
    use axum::{
        body::{Body, Bytes},
        extract::State,
        http::{Response, StatusCode},
    };

    /// Axum handler that delegates a request to the shared Fission renderer.
    pub async fn handle(
        State(renderer): State<Arc<ServerRenderer>>,
        request: axum::http::Request<axum::body::Body>,
    ) -> Response<Body> {
        let method = request.method().as_str().to_string();
        let path = request.uri().path().to_string();
        let query = parse_query(request.uri().query().unwrap_or(""));
        let headers = request
            .headers()
            .iter()
            .filter_map(|(name, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|value| (name.as_str().to_ascii_lowercase(), value.to_string()))
            })
            .collect();
        let body = axum::body::to_bytes(request.into_body(), 1024 * 1024)
            .await
            .unwrap_or_else(|_| Bytes::new())
            .to_vec();
        let response = renderer
            .handle(ServerRequest {
                method,
                path,
                query,
                headers,
                body,
            })
            .unwrap_or_else(|error| {
                ServerResponse::text(500, "text/plain; charset=utf-8", error.to_string())
            });
        let status =
            StatusCode::from_u16(response.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let mut builder = Response::builder().status(status);
        for (name, value) in &response.headers {
            builder = builder.header(name.as_str(), value.as_str());
        }
        builder.body(Body::from(response.body)).unwrap()
    }
}

#[cfg(feature = "actix-adapter")]
pub mod actix_adapter {
    use super::*;
    use actix_web::{web, HttpRequest, HttpResponse};

    /// Actix handler that delegates a request to the shared Fission renderer.
    pub async fn handle(
        renderer: web::Data<Arc<ServerRenderer>>,
        request: HttpRequest,
        body: web::Bytes,
    ) -> HttpResponse {
        let method = request.method().as_str().to_string();
        let path = request.path().to_string();
        let query = parse_query(request.query_string());
        let headers = request
            .headers()
            .iter()
            .filter_map(|(name, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|value| (name.as_str().to_ascii_lowercase(), value.to_string()))
            })
            .collect();
        let response = renderer
            .handle(ServerRequest {
                method,
                path,
                query,
                headers,
                body: body.to_vec(),
            })
            .unwrap_or_else(|error| {
                ServerResponse::text(500, "text/plain; charset=utf-8", error.to_string())
            });
        let mut builder = HttpResponse::build(
            actix_web::http::StatusCode::from_u16(response.status)
                .unwrap_or(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR),
        );
        for (name, value) in &response.headers {
            builder.insert_header((name.as_str(), value.as_str()));
        }
        builder.body(response.body)
    }
}

fn parse_query(query: &str) -> std::collections::BTreeMap<String, String> {
    let mut out = std::collections::BTreeMap::new();
    for part in query.split('&') {
        if part.is_empty() {
            continue;
        }
        let (key, value) = part.split_once('=').unwrap_or((part, ""));
        out.insert(key.to_string(), value.to_string());
    }
    out
}
