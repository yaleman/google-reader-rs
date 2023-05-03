use std::collections::HashMap;

use anyhow::Context;
use log::{debug, trace};
use reqwest::header::HeaderMap;
use reqwest::Client;
use serde::Deserialize;
use url::Url;

#[cfg(test)]
mod test;

#[derive(Debug)]
/// A Google Reader client
///
/// This should be instantiated with `GoogleReader::try_new()`, as a `mut` variable because login sets the authtoken.
pub struct GoogleReader {
    username: String,
    password: String,
    /// The server URL, e.g. `https://example.com/api/greader.php` for FreshRSS
    server_url: Url,
    authtoken: Option<String>,
    write_token: Option<String>,
    client: Option<Client>,
}

#[derive(Debug, Deserialize)]
/// A link to a resource
pub struct Link {
    pub href: String,
}

#[derive(Debug, Deserialize)]
/// Item Summary
pub struct Summary {
    pub content: Option<String>,
    pub author: Option<String>,
}

#[derive(Debug, Deserialize)]
/// Feed Item
pub struct Item {
    pub id: String,
    #[serde(alias = "crawlTimeMsec")]
    pub crawl_time_msec: Option<String>,
    #[serde(alias = "timestampUsec")]
    pub timestamp_usec: Option<String>,
    pub updated: Option<usize>,
    pub published: Option<usize>,
    pub title: String,
    pub canonical: Vec<Link>,
    pub alternate: Vec<Link>,
    pub categories: Vec<String>,
    pub origin: HashMap<String, String>,
    pub summary: Summary,
}

#[derive(Debug, Deserialize)]
/// Response from the API
pub struct Response {
    pub id: String,
    pub items: Vec<Item>,
    pub updated: usize,
    pub continuation: Option<String>,
}

/// Does all the things.
impl GoogleReader {
    /// The server URL is something like `https://example.com/api/greader.php` for FreshRSS
    pub fn try_new(
        username: impl ToString,
        password: impl ToString,
        server_url: impl ToString,
    ) -> anyhow::Result<Self> {
        let server_url = match server_url.to_string().ends_with('/') {
            true => server_url
                .to_string()
                .strip_suffix('/')
                .unwrap()
                .to_string(),
            false => server_url.to_string(),
        };

        let server_url = Url::parse(&server_url).with_context(|| "Failed to parse server URL")?;
        Ok(GoogleReader {
            username: username.to_string(),
            password: password.to_string(),
            server_url,
            authtoken: None,
            write_token: None,
            client: None,
        })
    }

    /// Do the login dance and cache the auth token.
    pub async fn login(&mut self) -> anyhow::Result<()> {
        let mut url = self.server_url.clone();
        url.path_segments_mut()
            .unwrap()
            .push("accounts")
            .push("ClientLogin");

        debug!("Login URL: {}", url);

        let params = [("Email", &self.username), ("Passwd", &self.password)];
        if self.client.is_none() {
            self.client = Some(reqwest::Client::new());
        }
        let res = self
            .client
            .as_ref()
            .unwrap()
            .post(url)
            .form(&params)
            .send()
            .await
            .with_context(|| "Failed to send login request")?;

        let auth_parser = regex::Regex::new(r#"Auth=(?P<authtoken>\S+)"#)
            .with_context(|| "Failed to generate auth parser regex")?;

        let body = res
            .text()
            .await
            .with_context(|| "Failed to get login response body")?;
        trace!("Login response: {}", body);

        let caps = auth_parser
            .captures(&body)
            .with_context(|| "Failed to parse login response")?;
        if let Some(authtoken) = caps.name("authtoken") {
            trace!("Got authtoken: {}", authtoken.as_str());
            self.authtoken = Some(authtoken.as_str().to_string());
        }

        Ok(())
    }

    /// Get a "write" token.
    pub async fn get_write_token(&mut self) -> anyhow::Result<String> {
        if self.authtoken.is_none() {
            self.login().await.with_context(|| "Failed to login")?;
        }
        let mut url = self.server_url.clone();
        url.path_segments_mut()
            .unwrap()
            .push("reader")
            .push("api")
            .push("0")
            .push("token");
        trace!("get_write_token url: {}", url);
        let res = self
            .client
            .as_ref()
            .unwrap()
            .get(url)
            .headers(self.get_auth_headers())
            .send()
            .await
            .with_context(|| "Failed to get write token")?;

        let mut body = res
            .text()
            .await
            .with_context(|| "Failed to get write token response body")?;

        if body.ends_with('\n') {
            body = body.strip_suffix('\n').unwrap().to_string();
        }

        self.write_token = Some(body.to_owned());

        Ok(body)
    }

    /// Returns a list of unread item IDs.
    pub async fn get_unread_items(
        &mut self,
        continuation: Option<String>,
    ) -> anyhow::Result<Response> {
        if self.authtoken.is_none() {
            self.login().await.with_context(|| "Failed to login")?;
        }

        // https://your-freshrss-instance-url/api/greader.php/reader/api/0/stream/contents/user/-/state/com.google/reading-list?ot=0&n=1000&r=n&xt=user/-/state/com.google/read

        let mut url = self.server_url.clone();
        url.path_segments_mut()
            .unwrap()
            .push("reader")
            .push("api")
            .push("0")
            .push("stream")
            .push("contents")
            .push("user")
            .push("-")
            .push("state")
            .push("com.google")
            .push("reading-list");
        /*
        ot=0: This is the "start time" for the request. Setting it to 0 means that you want to fetch all unread items since the beginning.
        n=1000: This parameter specifies the maximum number of items to fetch. You can adjust this value to the desired number of items.
        r=n: This parameter specifies the order in which items are returned. "n" stands for "newest first."
        xt=user/-/state/com.google/read: This parameter specifies that you want to exclude items that are already marked as read.
        */
        url.set_query(Some("r=n&xt=user/-/state/com.google/read"));
        if let Some(continuation) = continuation {
            url.set_query(Some(
                format!("c={}&{}", continuation, url.query().unwrap_or("")).as_str(),
            ))
        };
        trace!("url: {}", url);
        let res = self
            .client
            .as_ref()
            .unwrap()
            .get(url)
            .headers(self.get_auth_headers())
            .send()
            .await
            .with_context(|| "Failed to send unread-items request")?;

        let body = res
            .text()
            .await
            .with_context(|| "Failed to parse unread items response body")?;
        #[cfg(debug_assertions)]
        trace!("Response body:\n{}", body);
        let response: Response = serde_json::from_str(&body)
            .with_context(|| "Failed to parse unread items response body")?;
        debug!("response: {:#?}", response);

        Ok(response)
    }

    pub async fn get_item(&self, _item_id: usize) {}

    /// Returns the auth headers for use with the API.
    fn get_auth_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.append(
            "Authorization",
            format!("GoogleLogin auth={}", self.authtoken.clone().unwrap())
                .parse()
                .unwrap(),
        );
        #[cfg(debug_assertions)]
        trace!("Auth headers: {:?}", headers);
        headers
    }

    /// Mark an item as read
    pub async fn mark_item_read(&mut self, item_id: impl ToString) -> anyhow::Result<String> {
        if self.authtoken.is_none() {
            self.login().await.with_context(|| "Failed to login")?;
        }

        let write_token = match &self.write_token {
            Some(val) => val.to_owned(),
            None => self
                .get_write_token()
                .await
                .with_context(|| "Failed to get write token")?,
        };

        let params = [
            ("a", "user/-/state/com.google/read"),
            ("T", &write_token),
            ("i", &item_id.to_string()),
        ];

        let mut url = self.server_url.clone();
        url.path_segments_mut()
            .unwrap()
            .push("reader")
            .push("api")
            .push("0")
            .push("edit-tag");
        trace!("edit-tag url: {}", url);
        let res = self
            .client
            .as_ref()
            .unwrap()
            .post(url)
            .form(&params)
            .headers(self.get_auth_headers())
            .send()
            .await
            .with_context(|| "Failed to get write token")?;

        let body = res
            .text()
            .await
            .with_context(|| "Failed to get write token response body")?;

        Ok(body)
    }

    /// Returns the number of unread items, does'nt work for FreshRSS.
    pub async fn unread_count(&mut self) -> anyhow::Result<usize> {
        if self.authtoken.is_none() {
            self.login().await.with_context(|| "Failed to login")?;
        }

        let mut url = self.server_url.clone();
        url.path_segments_mut()
            .unwrap()
            .push("reader")
            .push("api")
            .push("0")
            .push("unread-count");
        #[cfg(debug_assertions)]
        trace!("url: {}", url);
        let res = self
            .client
            .as_ref()
            .unwrap()
            .get(url)
            .headers(self.get_auth_headers())
            .send()
            .await
            .with_context(|| "Failed to send unread-items request")?;

        let body = res
            .text()
            .await
            .with_context(|| "Failed to get unread count response body")?;

        let response_usize = body
            .parse::<usize>()
            .with_context(|| "Failed to parse unread count response")?;
        Ok(response_usize)
    }
}
