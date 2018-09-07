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
use http::Method;
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
        .map(|_| true)
        .unwrap_or_default()
}

/// Return true if provided authz header matches config
fn authenticated(config: &Config, authz: &str) -> bool {
    let payload = match &authz.split_whitespace().collect::<Vec<_>>()[..] {
        ["Basic", payload] => payload.clone(),
        _ => return false,
    };
    base64::decode(payload)
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
        .filter(|authz| authenticated(&config, authz.to_str().unwrap_or_default()))
        .is_none()
    {
        return Ok(Response::builder().status(401).body(())?);
    }

    match request.method() {
        &Method::GET | &Method::PUT => Ok(Response::builder()
            .status(307)
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
                200
            } else {
                404
            };
            Ok(Response::builder().status(status).body(())?)
        }
        _ => Ok(Response::builder().status(405).body(())?),
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
            "test"
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
            "Basic Zm9vOmJhcg=="
        ))
    }
}
