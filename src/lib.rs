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

fn head<C>(client: C, bucket: String, key: String) -> u16
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
        .ok()
        .map(|_| 200)
        .unwrap_or(404)
}

/// Return true if provided authz header matches config
fn authenticated(config: &Config, authz: &str) -> bool {
    let split = authz.split_whitespace().collect::<Vec<_>>();
    if split.len() != 2 {
        return false;
    }
    let (typ, payload) = (split[0], split[1]);
    if typ != "Basic" {
        return false;
    }
    if let Some(decoded) = base64::decode(payload).ok() {
        let Config {
            username, password, ..
        } = config;
        let decoded_str = String::from_utf8(decoded).unwrap_or_default();
        let split = decoded_str.split(':').collect::<Vec<_>>();
        if split.len() != 2 {
            return false;
        }
        let (user, pass) = (split[0], split[1]);
        return user == username && pass == password;
    }
    false
}

gateway!(|request, _| {
    let config = envy::from_env::<Config>()?;
    if request
        .headers()
        .get(AUTHORIZATION)
        .filter(|authz| authenticated(&config, authz.to_str()?))
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
            let status = head(
                S3Client::new(Default::default()),
                config.bucket,
                request.uri().path().into(),
            );
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
