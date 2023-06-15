use diesel::{deserialize::FromSql, pg::Pg, serialize::ToSql, sql_types::Bytea};
use rand::{distributions::Uniform, rngs::OsRng, Rng};
use sha2::{Digest, Sha256};

const TOKEN_LENGTH: usize = 32;

/// NEVER CHANGE THE PREFIX OF EXISTING TOKENS!!! Doing so will implicitly
/// revoke all the tokens, disrupting production users.
const TOKEN_PREFIX: &str = "cio";

#[derive(FromSqlRow, AsExpression)]
#[diesel(sql_type = Bytea)]
pub struct HashedToken {
    sha256: Vec<u8>,
}

impl HashedToken {
    pub(crate) fn parse(plaintext: &str) -> Option<Self> {
        // This will both reject tokens without a prefix and tokens of the wrong kind.
        if !plaintext.starts_with(TOKEN_PREFIX) {
            return None;
        }

        let sha256 = Self::hash(plaintext);
        Some(Self { sha256 })
    }

    pub fn hash(plaintext: &str) -> Vec<u8> {
        Sha256::digest(plaintext.as_bytes()).as_slice().to_vec()
    }
}

impl std::fmt::Debug for HashedToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HashedToken")
    }
}

impl ToSql<Bytea, Pg> for HashedToken {
    fn to_sql(&self, out: &mut diesel::serialize::Output<'_, '_, Pg>) -> diesel::serialize::Result {
        ToSql::<Bytea, Pg>::to_sql(&self.sha256, &mut out.reborrow())
    }
}

impl FromSql<Bytea, Pg> for HashedToken {
    fn from_sql(bytes: diesel::pg::PgValue<'_>) -> diesel::deserialize::Result<Self> {
        Ok(Self {
            sha256: FromSql::<Bytea, Pg>::from_sql(bytes)?,
        })
    }
}

pub(crate) struct NewSecureToken {
    plaintext: String,
}

impl NewSecureToken {
    pub(crate) fn generate() -> Self {
        let plaintext = format!(
            "{}{}",
            TOKEN_PREFIX,
            generate_secure_alphanumeric_string(TOKEN_LENGTH)
        );

        Self { plaintext }
    }

    pub(crate) fn plaintext(&self) -> &str {
        &self.plaintext
    }

    pub fn hashed(&self) -> HashedToken {
        let sha256 = HashedToken::hash(&self.plaintext);
        HashedToken { sha256 }
    }
}

fn generate_secure_alphanumeric_string(len: usize) -> String {
    const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

    OsRng
        .sample_iter(Uniform::from(0..CHARS.len()))
        .map(|idx| CHARS[idx] as char)
        .take(len)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generated_and_parse() {
        let token = NewSecureToken::generate();
        assert!(token.plaintext().starts_with(TOKEN_PREFIX));
        assert_eq!(
            token.hashed().sha256,
            Sha256::digest(token.plaintext().as_bytes()).as_slice()
        );

        let parsed = HashedToken::parse(token.plaintext()).expect("failed to parse back the token");
        assert_eq!(parsed.sha256, token.hashed().sha256);
    }

    #[test]
    fn test_parse_no_kind() {
        assert!(HashedToken::parse("nokind").is_none());
    }
}
