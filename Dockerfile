##
## srvr
##

# Base builder image
FROM rust:1.70-slim as builder

# Very nice
WORKDIR /usr/src/srvr

# Add the entire source
COPY . .

# We be building!
RUN --mount=type=cache,target=/usr/src/srvr/target \
    --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo build --release; \
    # move binary out of cached directory, so the runtime can copy it
    objcopy --compress-debug-sections target/release/srvr ./srvr

# Lean, mean, image machine
FROM gcr.io/distroless/cc as runtime

# Just the srvr binary
COPY --from=builder /usr/src/srvr/srvr /

# The volume srvr will be serving files from
VOLUME /var/srvr

# Tell the world srvr is running on port 80
EXPOSE 80

# Run, srvr, run!
ENTRYPOINT ["./srvr", "--address=0.0.0.0:80", "--port=80", "/var/srvr"]
