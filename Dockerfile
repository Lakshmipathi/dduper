FROM debian:bookworm-slim AS build
MAINTAINER Lakshmipathi.G

# Install build dependencies.
RUN apt-get update && apt-get install -y --no-install-recommends autoconf automake gcc \
    make pkg-config libext2fs-dev libblkid-dev zlib1g-dev liblzo2-dev \
    python3-dev libzstd-dev python3-pip python3-setuptools patch

# Clone btrfs-progs repo
ADD --checksum=sha256:ba1b8017b82e22f874fb3b09805ac4f45e3265888dc56eaf4449f38f896f6fc4 https://github.com/kdave/btrfs-progs/archive/refs/tags/v6.11.tar.gz /btrfs-progs.tar.gz

COPY patch/btrfs-progs-v6.11 /patch

# Apply csum patch (subvolume-aware version)
WORKDIR /btrfs-progs
RUN tar --strip-components 1 -xzf /btrfs-progs.tar.gz && \
    patch -p1 < /patch/0001-Print-csum-for-a-given-file-on-stdout-with-subvolume.patch

# Start the btrfs-progs build
RUN ./autogen.sh
# btrfs-progs 5.14 add an optional dependency (on by default) on libudev, for
# multipath device detection, but that requires a running udev daemon, and
# perhaps ohter changes to make it work inside a Docker container, so it's
# disabled for the moment
RUN ./configure --disable-documentation --disable-libudev
RUN make install DESTDIR=/btrfs-progs-build

# Start the btrfs-progs static build
RUN make clean
RUN make static
RUN make btrfs.static
RUN cp btrfs.static /btrfs-progs-build

# Install dduper
FROM debian:bookworm-slim
COPY --from=build /lib/x86_64-linux-gnu/liblzo2.so.2 /lib/x86_64-linux-gnu/
COPY --from=build /btrfs-progs-build /btrfs-progs
COPY . /dduper

RUN mv /btrfs-progs/btrfs.static /
RUN cp -rv /btrfs-progs/usr/local/bin/* /usr/local/bin && cp -rv /btrfs-progs/usr/local/include/* /usr/local/include/ && cp -rv /btrfs-progs/usr/local/lib/* /usr/local/lib
RUN btrfs inspect-internal dump-csum --help

WORKDIR /dduper

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends python3-pip python3-setuptools && \
    pip3 install --break-system-packages -r requirements.txt && \
    apt-get remove -y python3-pip python3-setuptools && \
    rm -rf /var/lib/apt/lists/* && \
    cp -v dduper /usr/sbin/ && \
    dduper --version
