# bazel s3 cache [![Build Status](https://travis-ci.com/meetup/bazel-s3-cache.svg?branch=master)](https://travis-ci.com/meetup/bazel-s3-cache) [![Coverage Status](https://coveralls.io/repos/github/meetup/bazel-s3-cache/badge.svg?branch=master)](https://coveralls.io/github/meetup/bazel-s3-cache?branch=master)

> a serverless implementation for a bazel build cache

## 🤔 about

Bazel is an input output machine. I can exploit the ability to avoid recompiling
what input combinations have already been compiled using a [remote caching](https://docs.bazel.build/versions/master/remote-caching.html) server. This repo
contains a serverless implementation of that protocol.

## 👩‍🏭 development

This is a [rustlang](https://www.rust-lang.org/en-US/) application.
Go grab yourself a copy with [rustup](https://rustup.rs/).

## 🚀 deployment

This is a rust application deployed using ⚡ [serverless](https://serverless.com/) ⚡.

> 💡 To install serverless, run `make dependencies`

This lambda is configured through its environment variables. Most of these have a default. `GITHUB_TOKEN` must be provided. Optional support for posting a slack message to
reviewers is supported by providing a `SLACK_TOKEN`

| Name          | Description                                      |
|---------------|--------------------------------------------------|
| `USERNAME`    | basic auth username                              |
| `PASSWORD`    | basic auth password                              |

Run `AWS_PROFILE=prod make deploy` to deploy.