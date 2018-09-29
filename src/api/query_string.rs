use std::fmt;
use openssl::{sign::Signer, hash::MessageDigest, pkey::{PKey, Private}};

crate struct QueryString {
    query: String,
}

impl QueryString {
    crate fn new() -> Self {
        QueryString {
            query: String::new(),
        }
    }

    crate fn push_str(&mut self, name: &str, arg: &str) {
        if !self.query.is_empty() {
            self.query.push('&');
        }
        self.query.push_str(name);
        self.query.push('=');
        self.query.push_str(arg);
    }

    crate fn push<P: fmt::Display>(&mut self, name: &str, arg: P) {
        use std::fmt::Write;

        if !self.query.is_empty() {
            self.query.push('&');
        }
        write!(&mut self.query, "{}={}", name, arg).unwrap();
    }

    crate fn into_string(self) -> String {
        self.query
    }

    crate fn into_string_with_signature(mut self, key: &PKey<Private>) -> String {
        let mut signer = Signer::new(MessageDigest::sha256(), key).unwrap();
        signer.update(self.query.as_bytes()).unwrap();
        let signature = hex::encode(&signer.sign_to_vec().unwrap());
        self.push("signature", &signature);
        self.query
    }
}
