Certificates for the TLS unit tests, made with OpenSSL, valid until 2126 except
`expired.pem` (2020-2021). `ca.pem` signs `server.pem` and `expired.pem`, both
for `localhost` and `127.0.0.1`; `other-ca.pem` signs neither. The keys protect
nothing but these tests.
