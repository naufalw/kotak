#!/bin/bash
set -e

ROOTFS="rootfs.ext4"
MOUNT="/tmp/my-rootfs"
DEST="$HOME/kotak/firecracker-local/rootfs.ext4"

echo "==> making ext4"
dd if=/dev/zero of=$ROOTFS bs=1M count=512
mkfs.ext4 $ROOTFS

echo "==> mounting"
mkdir -p $MOUNT
sudo mount $ROOTFS $MOUNT

echo "==> populating rootfs"
docker run --rm -v $MOUNT:/my-rootfs alpine sh -c '
    apk add openrc util-linux openssh bash

    # Serial Console
    ln -s agetty /etc/init.d/agetty.ttyS0
    echo ttyS0 > /etc/securetty
    rc-update add agetty.ttyS0 default

    # Boot services
    rc-update add devfs boot
    rc-update add procfs boot
    rc-update add sysfs boot

    # SSH services
    rc-update add sshd default
    echo "PermitRootLogin yes" >> /etc/ssh/sshd_config
    echo "PasswordAuthentication yes" >> /etc/ssh/sshd_config
    echo "root:root" | chpasswd
    ssh-keygen -A

    # DNS
    echo "nameserver 8.8.8.8" > /etc/resolv.conf
    echo "hosts: files dns" > /etc/nsswitch.conf

    # Filesystem copy
    for d in bin etc lib root sbin usr; do tar c "/$d" | tar x -C /my-rootfs; done
    for dir in dev proc run sys var; do mkdir -p /my-rootfs/${dir}; done
    mkdir -p /my-rootfs/var/empty
'

echo "==> unmount"
sudo umount $MOUNT

echo "==> copyy"
cp $ROOTFS $DEST

echo "==> done! rootfs here: $DEST"
