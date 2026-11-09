use crate::io::BufMutExt;
use crate::message::{FrontendMessage, FrontendMessageFormat};
use driver_core::Error;
use md5::{Digest, Md5};
use std::num::Saturating;

#[derive(Debug)]
pub enum Password<'a> {
    Cleartext(&'a str),

    Md5 {
        password: &'a str,
        username: &'a str,
        salt: [u8; 4],
    },
}

impl FrontendMessage for Password<'_> {
    const FORMAT: FrontendMessageFormat = FrontendMessageFormat::PasswordPolymorphic;

    #[inline(always)]
    fn body_size_hint(&self) -> Saturating<usize> {
        let mut size = Saturating(0);

        match self {
            Password::Cleartext(password) => {
                // To avoid reporting the exact password length anywhere,
                // we deliberately give a bad estimate.
                //
                // This shouldn't affect performance in the long run.
                size += password
                    .len()
                    .saturating_add(1) // NUL terminator
                    .checked_next_power_of_two()
                    .unwrap_or(usize::MAX);
            }
            Password::Md5 { .. } => {
                // "md5<32 hex chars>\0"
                size += 36;
            }
        }

        size
    }

    fn encode_body(&self, buf: &mut Vec<u8>) -> Result<(), Error> {
        match self {
            Password::Cleartext(password) => {
                buf.put_str_nul(password);
            }

            Password::Md5 {
                username,
                password,
                salt,
            } => {
                // The actual `PasswordMessage` can be computed in SQL as
                // `concat('md5', md5(concat(md5(concat(password, username)), random-salt)))`.

                // Keep in mind the md5() function returns its result as a hex string.

                let mut hasher = Md5::new();

                hasher.update(password);
                hasher.update(username);

                let mut output = hex::encode(hasher.finalize_reset());

                hasher.update(&output);
                hasher.update(salt);

                // This is not really optimal but hopefully this is an uncommon code path.
                // MD5 password hashing should really be phased out anyway.
                output = hex::encode(hasher.finalize());
                output.insert_str(0, "md5");

                buf.put_str_nul(&output);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
#[path = "../../tests/unit/message_password_tests.rs"]
mod tests;
