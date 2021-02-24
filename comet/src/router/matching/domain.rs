use std::str::FromStr;

use crate::prelude::*;
use anyhow::anyhow;
use regex::Regex;
use serde_with::DeserializeFromStr;

#[derive(Debug, Clone, DeserializeFromStr)]
pub enum DomainCondition {
    Regex(Regex),
    Keyword(SmolStr),
    Domain(SmolStr),
    Full(SmolStr),
}

impl FromStr for DomainCondition {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        if let Some(res) = s.strip_prefix("regex:") {
            let re = Regex::new(res)?;
            return Ok(Self::Regex(re));
        }

        if let Some(res) = s.strip_prefix("full:") {
            return Ok(Self::Full(res.into()));
        }

        if let Some(res) = s.strip_prefix("domain:") {
            if !res.is_ascii() {
                return Err(anyhow!("Non-ASCII chars in this rule"));
            }
            let res = format!(".{}", res);
            return Ok(Self::Domain(res.into()));
        }

        Ok(Self::Keyword(s.into()))
    }
}

impl DomainCondition {
    pub fn is_match(&self, domain: &str) -> bool {
        match self {
            DomainCondition::Regex(re) => re.is_match(domain),
            DomainCondition::Keyword(kw) => domain.contains(kw.as_str()),
            DomainCondition::Domain(dm) => domain == &dm[1..] || domain.ends_with(dm.as_str()),
            DomainCondition::Full(dm) => domain == dm,
        }
    }
}
