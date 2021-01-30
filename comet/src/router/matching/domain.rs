use std::borrow::Borrow;

use crate::prelude::*;
use regex::RegexSet;
use serde::de::Error;
use serde::{Deserialize, Deserializer};

#[derive(Debug, Clone, Deserialize)]
pub struct DomainMatcher {
    /// `regex:`
    #[serde(deserialize_with = "serde_regex::deserialize")]
    regex: RegexSet,
    /// `full:`
    full: Vec<SmolStr>,
    /// Rules without prefix
    contains: Vec<SmolStr>,
    /// `domain:`
    domain: Vec<SmolStr>,
}

impl DomainMatcher {
    pub fn new<I>(rules: I) -> Result<Self>
    where
        I: IntoIterator<Item = SmolStr>,
    {
        let mut regex: Vec<String> = vec![];
        let mut full = vec![];
        let mut contains = vec![];
        let mut domain = vec![];

        for rule in rules {
            if let Some(res) = rule.strip_prefix("regex:") {
                regex.push(res.into());
                continue;
            }

            if let Some(res) = rule.strip_prefix("full:") {
                full.push(res.into());
                continue;
            }

            if let Some(res) = rule.strip_prefix("domain:") {
                domain.push(format!(".{}", res).into());
                continue;
            }

            contains.push(rule);
        }
        Ok(DomainMatcher {
            regex: RegexSet::new(regex)?,
            full,
            contains,
            domain,
        })
    }

    pub fn is_match(&self, domain: &str) -> bool {
        for pat in &self.contains {
            let pat: &str = pat.borrow();
            if domain.contains(pat) {
                return true;
            }
        }

        for pat in &self.full {
            let pat: &str = pat.borrow();
            if domain == pat {
                return true;
            }
        }

        for pat in &self.domain {
            let pat: &str = pat.borrow();
            if domain.ends_with(pat) {
                return true;
            }
        }
        if self.regex.is_match(domain) {
            return true;
        }
        false
    }
}

pub fn deserialize_domain_matcher_text<'de, D>(d: D) -> Result<DomainMatcher, D::Error>
where
    D: Deserializer<'de>,
{
    let rules = <Vec<SmolStr>>::deserialize(d)?;

    match DomainMatcher::new(rules) {
        Ok(r) => Ok(r),
        Err(err) => Err(D::Error::custom(err)),
    }
}
