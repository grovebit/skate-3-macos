#!/bin/bash
# Prints the path of a pinned setup tool, installing it first, so that every
# Mac sets up the game with the same tools:
#
#   tools/pinned_tool.sh uv          uv
#   tools/pinned_tool.sh python      the Python of .local/venv-setup: .python-version
#                                    with the hash-locked tools/requirements-setup.txt
#   tools/pinned_tool.sh vgmstream   vgmstream-cli, which decodes the original sounds
#
# Downloads must match their pinned SHA-256. A tool in .local/tools is
# installed again when its install function below changes.
set -euo pipefail

cd "$(dirname "$0")/.."
tools=$PWD/.local/tools
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

fail() { printf '\033[31m%s\033[0m\n' "$*" >&2; exit 1; }

# Downloads $1 to $3, which must have the SHA-256 $2.
fetch() {
    curl --proto '=https' --tlsv1.2 -fsSL "$1" -o "$3"
    shasum -a 256 -c --status <<<"$2  $3" || fail "$1 does not match its pinned checksum."
}

install_uv() {
    local version=0.12.23 sha256=50487ae565ccd96e499056b4674d438f4c53170202617b4c759defe0c6a1b544
    echo "Downloading uv $version." >&2
    fetch "https://github.com/astral-sh/uv/releases/download/$version/uv-aarch64-apple-darwin.tar.gz" "$sha256" "$work/uv.tar.gz"
    tar -xzf "$work/uv.tar.gz" -C "$1" --strip-components 1
}

# vgmstream with a static FFmpeg, built with the Xcode Command Line Tools.
install_vgmstream() {
    local version=r2117 commit=71e2361042531fe767fb98300cf8c1ee95e539a0
    local sha256=9bb2281f4644eeb7a99d400c03242c5c99900f205989816fd2a76a4946dcf00a
    local ffmpeg=9.0.2 ffmpeg_sha256=8c3850283eb25fa026482078a04051e0be17347b09ef81a0849bec15a96e002e
    local jobs
    jobs=$(sysctl -n hw.ncpu)
    echo "Building vgmstream $version with FFmpeg $ffmpeg to decode the original sounds; it takes about a minute." >&2
    fetch "https://ffmpeg.org/releases/ffmpeg-$ffmpeg.tar.xz" "$ffmpeg_sha256" "$work/ffmpeg.tar.xz"
    fetch "https://github.com/vgmstream/vgmstream/archive/$commit.tar.gz" "$sha256" "$work/vgmstream.tar.gz"
    mkdir "$work/ffmpeg" "$work/vgmstream"
    tar -xJf "$work/ffmpeg.tar.xz" -C "$work/ffmpeg" --strip-components 1
    tar -xzf "$work/vgmstream.tar.gz" -C "$work/vgmstream" --strip-components 1
    # Only the FFmpeg components that vgmstream's own static builds enable
    # (its cmake/dependencies/ffmpeg.cmake).
    (cd "$work/ffmpeg" && ./configure --prefix="$work/ffmpeg-build" --disable-everything \
        --disable-autodetect --disable-programs --disable-doc --disable-network \
        --disable-avdevice --disable-avfilter --disable-swscale \
        --enable-demuxer=ac3,eac3,spdif,asf,xwma,mov,oma,xmv,ogg,flac,wav,aac,mp3,smacker,bink,binka,caf,mpc,mpc8,tak,ape \
        --enable-parser=ac3,mpegaudio,xma,vorbis,opus \
        --enable-decoder=ac3,eac3,wmapro,wmav1,wmav2,xma1,xma2,aac,atrac3,atrac3p,mp2float,mp3float,smackaud,binkaudio_dct,binkaudio_rdft,pcm_s16be,pcm_s16be_planar,pcm_s16le,pcm_s16le_planar,pcm_s8,pcm_s8_planar,flac,vorbis,mpc7,mpc8,alac,adpcm_ima_qt,adpcm_ima_dk3,adpcm_ima_dk4,tak,ape,opus &&
        make -j"$jobs" install) >"$work/ffmpeg.log" 2>&1 ||
        { tail -n 20 "$work/ffmpeg.log" >&2; fail "FFmpeg did not build."; }
    # The command-line tool's own Makefile, given the flags of vgmstream's CMake
    # release builds; its top-level Makefile would add -ffast-math.
    make -C "$work/vgmstream/cli" -j"$jobs" vgmstream_cli CC=cc AR=ar STRIP=strip RMF='rm -f' \
        DEF_CFLAGS='-O3 -DNDEBUG -DVGM_LOG_OUTPUT' \
        LIBS_CFLAGS="-DVGM_USE_FFMPEG -DVGM_USE_G7221 -I$work/ffmpeg-build/include" \
        LIBS_LDFLAGS="-L$work/ffmpeg-build/lib -lavformat -lavcodec -lswresample -lavutil" \
        >"$work/vgmstream.log" 2>&1 ||
        { tail -n 20 "$work/vgmstream.log" >&2; fail "vgmstream did not build."; }
    mv "$work/vgmstream/cli/vgmstream-cli" "$1"
}

# Installs .local/tools/$1 with install_$1, unless that function, pins
# included, already built the copy there.
tool() {
    local dir=$tools/$1 recipe
    recipe=$(declare -f "install_$1" | shasum -a 256 | cut -d' ' -f1)
    [ "$(cat "$dir/recipe" 2>/dev/null)" != "$recipe" ] || return 0
    mkdir "$work/out"
    "install_$1" "$work/out"
    echo "$recipe" >"$work/out/recipe"
    rm -rf "$dir"
    mkdir -p "$tools"
    mv "$work/out" "$dir"
}

# .local/venv-setup with Python from .python-version and the hash-locked
# packages. The interpreter lives beside uv, so a new uv pin replaces it too.
setup_python() {
    local venv=$PWD/.local/venv-setup version config
    version=$(<.python-version)
    export UV_NO_CONFIG=1 UV_PYTHON_INSTALL_DIR=$tools/uv/python
    config=$(cat "$venv/pyvenv.cfg" 2>/dev/null || true)
    if [ ! -x "$venv/bin/python" ] || [[ $config != *"version_info = $version"* ||
        $config != *"home = $UV_PYTHON_INSTALL_DIR/"* ]]; then
        echo "Installing Python $version for the setup tools." >&2
        "$tools/uv/uv" venv --quiet --clear --managed-python --python "$version" "$venv"
    fi
    "$tools/uv/uv" pip install --quiet --require-hashes --python "$venv/bin/python" \
        -r tools/requirements-setup.txt
    echo "$venv/bin/python"
}

case ${1-} in
    uv) tool uv; echo "$tools/uv/uv" ;;
    python) tool uv; setup_python ;;
    vgmstream) tool vgmstream; echo "$tools/vgmstream/vgmstream-cli" ;;
    *) fail "Usage: tools/pinned_tool.sh uv|python|vgmstream" ;;
esac
