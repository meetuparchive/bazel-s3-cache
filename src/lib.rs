// todo: s3 bucket lifecycle management
//
#[macro_use]
extern crate cpython;
#[macro_use]
extern crate lando;
extern crate rusoto_core;
extern crate rusoto_s3;
#[macro_use]
extern crate serde_derive;
extern crate base64;
extern crate envy;
extern crate futures;
extern crate http;

use std::time::Duration;

use futures::future::Future;
use http::header::{AUTHORIZATION, LOCATION};
use http::{Method, StatusCode};
use lando::Response;
use rusoto_core::credential::{AwsCredentials, ChainProvider, ProvideAwsCredentials};
use rusoto_s3::util::PreSignedRequest;
use rusoto_s3::{GetObjectRequest, HeadObjectRequest, PutObjectRequest, S3, S3Client};

#[derive(Deserialize, Default)]
struct Config {
    bucket: String,
    username: String,
    password: String,
}

fn credentials() -> ChainProvider {
    let mut chain = ChainProvider::new();
    chain.set_timeout(Duration::from_millis(200));
    chain
}

fn get(bucket: String, key: String, credentials: &AwsCredentials) -> String {
    GetObjectRequest {
        bucket,
        key,
        ..Default::default()
    }.get_presigned_url(&Default::default(), &credentials)
}

fn put(bucket: String, key: String, credentials: &AwsCredentials) -> String {
    PutObjectRequest {
        bucket,
        key,
        ..Default::default()
    }.get_presigned_url(&Default::default(), &credentials)
}

fn exists<C>(client: C, bucket: String, key: String) -> bool
where
    C: S3,
{
    client
        .head_object(HeadObjectRequest {
            bucket,
            key,
            ..Default::default()
        })
        .sync()
        .is_ok()
}

/// Return true if provided authz header matches config
fn authenticated(config: &Config, authz: &[u8]) -> bool {
    if authz
        .get(..6)
        .filter(|prefix| prefix == b"Basic ")
        .is_none()
    {
        return false;
    }
    base64::decode(&authz[6..])
        .ok()
        .into_iter()
        .filter_map(|bytes| String::from_utf8(bytes).ok())
        .any(|decoded| {
            let Config {
                username, password, ..
            } = config;
            match &decoded.split(':').collect::<Vec<_>>()[..] {
                [user, pass] => user == username && pass == password,
                _ => false,
            }
        })
}

gateway!(|request, _| {
    let config = envy::from_env::<Config>()?;
    if request
        .headers()
        .get(AUTHORIZATION)
        .filter(|authz| authenticated(&config, authz.as_bytes()))
        .is_none()
    {
        return Ok(Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .body(())?);
    }

    match request.method() {
        &Method::GET | &Method::PUT => Ok(Response::builder()
            .status(StatusCode::TEMPORARY_REDIRECT)
            .header(
                LOCATION,
                match request.method() {
                    &Method::GET => get(
                        config.bucket,
                        request.uri().path().into(),
                        &credentials().credentials().wait()?,
                    ),
                    _ => put(
                        config.bucket,
                        request.uri().path().into(),
                        &credentials().credentials().wait()?,
                    ),
                },
            )
            .body(())?),
        &Method::HEAD => {
            let status = if exists(
                S3Client::new(Default::default()),
                config.bucket,
                request.uri().path().into(),
            ) {
                StatusCode::OK
            } else {
                StatusCode::NOT_FOUND
            };
            Ok(Response::builder().status(status).body(())?)
        }
        _ => Ok(Response::builder()
            .status(StatusCode::METHOD_NOT_ALLOWED)
            .body(())?),
    }
});

#[cfg(test)]
mod tests {
    use super::{authenticated, Config};

    #[test]
    fn authenticated_rejects_invalid_requests() {
        assert!(!authenticated(
            &Config {
                username: "foo".into(),
                password: "bar".into(),
                ..Default::default()
            },
            "test".as_bytes()
        ))
    }

    #[test]
    fn authenticated_rejects_partially_valid_requests() {
        assert!(!authenticated(
            &Config {
                username: "foo".into(),
                password: "bar".into(),
                ..Default::default()
            },
            "Basic Zm9v".as_bytes()
        ))
    }

    #[test]
    fn authenticated_permits_valid_requests() {
        assert!(authenticated(
            &Config {
                username: "foo".into(),
                password: "bar".into(),
                ..Default::default()
            },
            "Basic Zm9vOmJhcg==".as_bytes()
        ))
    }
}
