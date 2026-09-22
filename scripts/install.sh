#!/bin/sh
# Install ClearQuota from the latest GitHub release.
# This file is published as https://github.com/sz-tamas/clearquota/releases/latest/download/install.sh.

set -eu

repository="sz-tamas/clearquota"
release_url="https://github.com/${repository}/releases/latest/download"
data_home="${XDG_DATA_HOME:-$HOME/.local/share}"
install_dir="${CLEARQUOTA_INSTALL_DIR:-$data_home/clearquota}"
bin_dir="${CLEARQUOTA_BIN_DIR:-$HOME/.local/bin}"

fail() {
	printf '%s\n' "clearquota installer: $*" >&2
	exit 1
}

case "$(uname -s)" in
	Linux) platform="linux" ;;
	Darwin) platform="macos" ;;
	*) fail "unsupported operating system: $(uname -s)" ;;
esac

case "$(uname -m)" in
	x86_64|amd64) architecture="x86_64" ;;
	arm64|aarch64) architecture="aarch64" ;;
	*) fail "unsupported architecture: $(uname -m)" ;;
esac

archive="clearquota_${platform}_${architecture}.tar.gz"
checksum_file="SHA256SUMS"
temporary_dir="$(mktemp -d 2>/dev/null || mktemp -d -t clearquota)"
trap 'rm -rf "$temporary_dir"' EXIT HUP INT TERM

command -v curl >/dev/null 2>&1 || fail "curl is required"
command -v tar >/dev/null 2>&1 || fail "tar is required"
command -v gcloud >/dev/null 2>&1 || fail "Google Cloud CLI (gcloud) is required. Install it from https://cloud.google.com/sdk/docs/install, then run this installer again."

printf 'Downloading ClearQuota for %s/%s...\n' "$platform" "$architecture"
curl --fail --location --silent --show-error \
	"${release_url}/${archive}" --output "${temporary_dir}/${archive}"
curl --fail --location --silent --show-error \
	"${release_url}/${checksum_file}" --output "${temporary_dir}/${checksum_file}"

expected_checksum="$(awk -v file="$archive" '$2 == file { print $1; exit }' "${temporary_dir}/${checksum_file}")"
[ -n "$expected_checksum" ] || fail "no checksum was published for ${archive}"

if command -v sha256sum >/dev/null 2>&1; then
	actual_checksum="$(sha256sum "${temporary_dir}/${archive}" | awk '{ print $1 }')"
elif command -v shasum >/dev/null 2>&1; then
	actual_checksum="$(shasum -a 256 "${temporary_dir}/${archive}" | awk '{ print $1 }')"
else
	fail "sha256sum or shasum is required to verify the download"
fi
[ "$expected_checksum" = "$actual_checksum" ] || fail "checksum verification failed"

mkdir -p "$install_dir" "$bin_dir"
tar -xzf "${temporary_dir}/${archive}" -C "$install_dir"
chmod 755 "${install_dir}/clearquota"

cat > "${bin_dir}/clearquota" <<EOF
#!/bin/sh
cd "${install_dir}"
exec "${install_dir}/clearquota" "\$@"
EOF
chmod 755 "${bin_dir}/clearquota"

printf 'ClearQuota installed to %s\n' "$install_dir"
printf 'Run %s/clearquota, then open http://127.0.0.1:3000\n' "$bin_dir"
case ":$PATH:" in
	*":${bin_dir}:"*) ;;
	*) printf 'Add %s to your PATH to run clearquota directly.\n' "$bin_dir" ;;
esac
