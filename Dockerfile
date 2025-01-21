FROM jimmydottech/jimmy:1.0-base AS builder

COPY . /jimmy

RUN --mount=type=secret,id=gitconfig,target=/root/.gitconfig \
    --mount=type=secret,id=enclave-key,target=/root/.config/gramine/enclave-key.pem \
    bash -c "cd /jimmy && SGX=1 make"

RUN mkdir -p /compiled/target/release
RUN cp /jimmy/jimmy.manifest /compiled
RUN cp /jimmy/jimmy.manifest.sgx /compiled
RUN cp /jimmy/jimmy.sig /compiled
RUN cp /jimmy/target/release/jimmy /compiled/target/release

RUN cp /jimmy/ca-certificates.crt /compiled
RUN cp -r /jimmy/assets /compiled

RUN gramine-sgx-sigstruct-view /compiled/jimmy.sig


FROM jimmydottech/jimmy:1.0-base AS runtime

COPY --from=builder /compiled /jimmy

RUN echo "#!/bin/bash" > /run.sh

RUN echo "/restart_aesm.sh && cd /jimmy && gramine-sgx-sigstruct-view jimmy.sig && mkdir -p store && gramine-sgx jimmy" >> /run.sh
RUN chmod +x /run.sh
ENTRYPOINT ["/run.sh"]