//! Bounded daily publisher. Uses the same article extractor as the native reader.
use anyhow::{ensure, Context, Result};
use futures::{stream, StreamExt};
use hn_lib::{
    hn_client::{HackerNewsClient, HackerNewsClientImpl},
    snapshot::{CapturedArticle, Snapshot},
    HackerNewsCliServiceImpl,
};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::Path,
    time::Duration,
};

pub async fn capture(path: &Path) -> Result<()> {
    capture_with(HackerNewsClientImpl::new(), path).await
}

async fn capture_with(client: impl HackerNewsClient + Clone, path: &Path) -> Result<()> {
    let mut feeds = BTreeMap::new();
    for category in ["top", "new", "best"] {
        let ids = client.get_story_ids(category).await?;
        feeds.insert(
            category.to_string(),
            ids.into_iter().take(20).collect::<Vec<_>>(),
        );
    }
    let unique: BTreeSet<_> = feeds.values().flatten().copied().collect();
    let mut items = BTreeMap::new();
    let mut articles = BTreeMap::new();
    let mut captures = stream::iter(unique.into_iter().map(|id| {
        let client = client.clone();
        async move {
            let story = client.get_item(id).await?;
            let service = HackerNewsCliServiceImpl::new_with_client(client.clone());
            let cli = service.story_from_item(story.clone());
            let article = match public_article_client(&cli.url).await {
                Ok(http) => match hn_lib::reader::sources::article(&http, &cli).await {
                    Ok((text, partial)) => CapturedArticle {
                        text,
                        partial,
                        error: None,
                        summary: None,
                    },
                    Err(e) => CapturedArticle {
                        text: String::new(),
                        partial: false,
                        error: Some(e.message),
                        summary: None,
                    },
                },
                Err(e) => CapturedArticle {
                    text: String::new(),
                    partial: false,
                    error: Some(e.to_string()),
                    summary: None,
                },
            };
            let mut queue: VecDeque<_> = story.kids.clone().unwrap_or_default().into();
            let mut captured = vec![story];
            let mut seen = BTreeSet::from([id]);
            for _ in 0..20 {
                let Some(id) = queue.pop_front() else { break };
                if !seen.insert(id) {
                    continue;
                }
                if let Ok(comment) = client.get_item(id).await {
                    // Parent-first depth-first traversal preserves some real reply threads.
                    for child in comment.kids.iter().flatten().rev() {
                        queue.push_front(*child);
                    }
                    captured.push(comment);
                }
            }
            eprintln!(
                "Captured {id}: {} comments, {} article characters{}",
                captured.len() - 1,
                article.text.chars().count(),
                if article.error.is_some() {
                    " (unavailable)"
                } else {
                    ""
                }
            );
            Ok::<_, anyhow::Error>((id, captured, article))
        }
    }))
    .buffer_unordered(6);
    while let Some(result) = captures.next().await {
        let (id, captured, article) = result?;
        for mut item in captured {
            item.title = hn_lib::reader::sources::clean(&item.title);
            item.text = item.text.map(|s| hn_lib::reader::sources::clean(&s));
            items.insert(item.id, item);
        }
        articles.insert(id, article);
    }
    let captured_ids: BTreeSet<_> = items.keys().copied().collect();
    for item in items.values_mut() {
        if let Some(kids) = &mut item.kids {
            kids.retain(|id| captured_ids.contains(id));
        }
    }
    let snapshot = Snapshot {
        version: 1,
        generated_at: chrono::Utc::now().to_rfc3339(),
        feeds,
        items,
        articles,
    };
    snapshot.write_atomic(path)?;
    eprintln!(
        "Published {} stories to {}",
        snapshot.articles.len(),
        path.display()
    );
    Ok(())
}

async fn public_article_client(raw: &str) -> Result<reqwest::Client> {
    let url = reqwest::Url::parse(raw)?;
    ensure!(
        url.scheme() == "https" && url.port_or_known_default() == Some(443),
        "Preview captures HTTPS articles only"
    );
    ensure!(
        url.username().is_empty() && url.password().is_none(),
        "Credentials in URL rejected"
    );
    let host = url.host_str().context("Missing article host")?;
    let addresses: Vec<_> =
        tokio::time::timeout(Duration::from_secs(5), tokio::net::lookup_host((host, 443)))
            .await??
            .filter(|a| match a.ip() {
                std::net::IpAddr::V4(ip) => {
                    let [a, b, _, _] = ip.octets();
                    !(ip.is_private()
                        || ip.is_loopback()
                        || ip.is_link_local()
                        || ip.is_unspecified()
                        || ip.is_broadcast()
                        || ip.is_documentation()
                        || ip.is_multicast()
                        || a == 0
                        || a >= 240
                        || (a == 100 && (64..=127).contains(&b))
                        || (a == 198 && (b == 18 || b == 19)))
                }
                std::net::IpAddr::V6(_) => false,
            })
            .collect();
    ensure!(!addresses.is_empty(), "No public IPv4 article address");
    // Pin the checked addresses; disable proxies and redirects so they cannot bypass this check.
    Ok(reqwest::Client::builder()
        .no_proxy()
        .resolve_to_addrs(host, &addresses)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(12))
        .user_agent("hn-cli-preview/0.1")
        .build()?)
}

/// Enrich a validated capture without replacing it if any selected summary fails.
pub async fn prepare(path: &Path) -> Result<()> {
    let mut snapshot = Snapshot::load(path)?;
    hn_lib::reader::snapshot::prepare(&mut snapshot).await?;
    snapshot.write_atomic(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use hn_lib::hn_client::HackerNewsItem;
    use std::{
        fs,
        sync::{Arc, Mutex},
    };

    #[derive(Clone)]
    struct CountingClient {
        items: BTreeMap<i32, HackerNewsItem>,
        requests: Arc<Mutex<BTreeMap<i32, usize>>>,
    }

    #[async_trait]
    impl HackerNewsClient for CountingClient {
        async fn get_story_ids(&self, story_type: &str) -> Result<Vec<i32>> {
            Ok(match story_type {
                "top" => vec![1, 2],
                "new" => vec![2, 3],
                "best" => vec![3, 1],
                _ => anyhow::bail!("Unexpected feed {story_type}"),
            })
        }

        async fn get_items(&self, ids: &[i32]) -> Vec<Result<HackerNewsItem>> {
            let mut items = Vec::new();
            for &id in ids {
                items.push(self.get_item(id).await);
            }
            items
        }

        async fn get_item(&self, id: i32) -> Result<HackerNewsItem> {
            *self.requests.lock().unwrap().entry(id).or_default() += 1;
            self.items
                .get(&id)
                .cloned()
                .with_context(|| format!("Unexpected item {id}"))
        }

        fn get_y_combinator_url(&self) -> &str {
            "http://example.test/"
        }
    }

    #[tokio::test]
    async fn capture_fetches_each_story_once() -> Result<()> {
        // HTTP URLs fail the article HTTPS check before hostname resolution.
        let items: [HackerNewsItem; 4] = serde_json::from_str(
            r#"[
            {"id":1,"type":"story","time":1,"title":"First","url":"http://example.test/story","kids":[4]},
            {"id":2,"type":"story","time":1,"title":"Second","url":"http://example.test/story"},
            {"id":3,"type":"story","time":1,"title":"Third","url":"http://example.test/story"},
            {"id":4,"type":"comment","time":1,"text":"A reply"}
        ]"#,
        )?;
        let requests = Arc::new(Mutex::new(BTreeMap::new()));
        let client = CountingClient {
            items: items.into_iter().map(|item| (item.id, item)).collect(),
            requests: Arc::clone(&requests),
        };
        let dir = std::env::temp_dir().join(format!(
            "hn-snapshot-capture-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_micros()
        ));
        fs::create_dir(&dir)?;
        let path = dir.join("snapshot.json");
        let captured = capture_with(client, &path)
            .await
            .and_then(|()| Snapshot::load(&path));
        fs::remove_dir_all(&dir)?;
        let snapshot = captured?;

        let mut counts = requests.lock().unwrap();
        assert_eq!(counts.remove(&4), Some(1), "comment fetch count");
        assert_eq!(
            *counts,
            BTreeMap::from([(1, 1), (2, 1), (3, 1)]),
            "story fetch counts"
        );
        assert_eq!(snapshot.items[&1].kids.as_deref(), Some([4].as_slice()));
        assert_eq!(snapshot.items[&4].r#type, "comment");
        assert_eq!(
            snapshot.articles.keys().copied().collect::<Vec<_>>(),
            [1, 2, 3]
        );
        assert!(snapshot.articles.values().all(|article| {
            article.error.as_deref() == Some("Preview captures HTTPS articles only")
        }));
        Ok(())
    }
}
