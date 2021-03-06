//! Variable server parameters lists

use crate::provider::{Protocol, Socket};

/// Set of variable parameters to try during configuration.
///
/// Can be loaded from offline provider database, online configuraiton
/// or derived from user entered parameters.
#[derive(Debug, Clone)]
pub(crate) struct ServerParams {
    /// Protocol, such as IMAP or SMTP.
    pub protocol: Protocol,

    /// Server hostname, empty if unknown.
    pub hostname: String,

    /// Server port, zero if unknown.
    pub port: u16,

    /// Socket security, such as TLS or STARTTLS, Socket::Automatic if unknown.
    pub socket: Socket,

    /// Username, empty if unknown.
    pub username: String,
}

impl ServerParams {
    pub(crate) fn expand_usernames(mut self, addr: &str) -> Vec<ServerParams> {
        let mut res = Vec::new();

        if self.username.is_empty() {
            self.username = addr.to_string();
            res.push(self.clone());

            if let Some(at) = addr.find('@') {
                self.username = addr.split_at(at).0.to_string();
                res.push(self);
            }
        } else {
            res.push(self)
        }
        res
    }

    pub(crate) fn expand_hostnames(mut self, param_domain: &str) -> Vec<ServerParams> {
        let mut res = Vec::new();
        if self.hostname.is_empty() {
            self.hostname = param_domain.to_string();
            res.push(self.clone());

            self.hostname = match self.protocol {
                Protocol::IMAP => "imap.".to_string() + param_domain,
                Protocol::SMTP => "smtp.".to_string() + param_domain,
            };
            res.push(self.clone());

            self.hostname = "mail.".to_string() + param_domain;
            res.push(self);
        } else {
            res.push(self);
        }
        res
    }

    pub(crate) fn expand_ports(mut self) -> Vec<ServerParams> {
        // Try to infer port from socket security.
        if self.port == 0 {
            self.port = match self.socket {
                Socket::SSL => match self.protocol {
                    Protocol::IMAP => 993,
                    Protocol::SMTP => 465,
                },
                Socket::STARTTLS | Socket::Plain => match self.protocol {
                    Protocol::IMAP => 143,
                    Protocol::SMTP => 587,
                },
                Socket::Automatic => 0,
            }
        }

        let mut res = Vec::new();
        if self.port == 0 {
            // Neither port nor security is set.
            //
            // Try common secure combinations.

            // Try STARTTLS
            self.socket = Socket::STARTTLS;
            self.port = match self.protocol {
                Protocol::IMAP => 143,
                Protocol::SMTP => 587,
            };
            res.push(self.clone());

            // Try TLS
            self.socket = Socket::SSL;
            self.port = match self.protocol {
                Protocol::IMAP => 993,
                Protocol::SMTP => 465,
            };
            res.push(self);
        } else if self.socket == Socket::Automatic {
            // Try TLS over user-provided port.
            self.socket = Socket::SSL;
            res.push(self.clone());

            // Try STARTTLS over user-provided port.
            self.socket = Socket::STARTTLS;
            res.push(self);
        } else {
            res.push(self);
        }
        res
    }
}
