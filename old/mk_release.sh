#!/bin/bash
# simple helper script to keep server and client in sync for playground

rm -R build_linux_rel
rm -R build_win_rel

mkdir build_linux_rel
mkdir build_win_rel

(
    cd build_linux_rel
    cmake .. -GNinja -DDOWNLOAD_GTEST=OFF
    ninja package_default
)

(
    cd build_win_rel
    cmake .. -GNinja -DCMAKE_TOOLCHAIN_FILE=../cmake/toolchains/mingw64.toolchain -DDOWNLOAD_GTEST=OFF
    ninja package_default
)

cp build_linux_rel/DDNet-Server ~/ddnet_run
cp build_linux_rel/DDNet-*.tar.xz /var/www/html/downloads/DDNet.tar.xz
cp build_win_rel/DDNet-*.zip /var/www/html/downloads/DDNet.zip
