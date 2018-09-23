use std::fmt;
use openssl::{sign::Signer, hash::MessageDigest, pkey::{PKey, Private}};

pub struct QueryString {
    query: String,
}

impl QueryString {
    pub fn new() -> Self {
        QueryString {
            query: String::new(),
        }
    }

    pub fn push<P: fmt::Display>(&mut self, name: &str, arg: P) {
        use std::fmt::Write;

        if self.query.is_empty() {
            write!(&mut self.query, "{}={}", name, arg).unwrap();
        } else {
            write!(&mut self.query, "&{}={}", name, arg).unwrap();
        }
    }

    pub fn into_string(self) -> String {
        self.query
    }

    pub fn into_string_with_signature(mut self, key: &PKey<Private>) -> String {
        let mut signer = Signer::new(MessageDigest::sha256(), key).unwrap();
        signer.update(self.query.as_bytes()).unwrap();
        let signature = hex::encode(&signer.sign_to_vec().unwrap());
        self.push("signature", &signature);
        self.query
    }
}
