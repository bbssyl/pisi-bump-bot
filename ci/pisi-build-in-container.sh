#!/bin/bash
set -uo pipefail

recipe_path="${1:?recipe yolu gerekli}"
full_upgrade="${2:-false}"
output_dir=/out
log_file="$output_dir/build.log"

mkdir -p "$output_dir"
: > "$log_file"

log_phase() {
  echo "== $1 ==" | tee -a "$log_file"
}

start_dbus() {
  log_phase "D-Bus başlatılıyor"
  service dbus start 2>&1 | tee -a "$log_file" || true
}

has_repo() {
  pisi lr 2>/dev/null | grep -qi "^$1\b"
}

add_repo_if_missing() {
  local repo_name="$1"
  local repo_url="$2"
  if has_repo "$repo_name"; then
    echo "Depo zaten var: $repo_name" | tee -a "$log_file"
    return 0
  fi
  echo "Depo ekleniyor: $repo_name" | tee -a "$log_file"
  pisi ar "$repo_name" "$repo_url" --ignore-check 2>&1 | tee -a "$log_file" || true
}

configure_repos() {
  log_phase "Depolar ayarlanıyor"
  echo "-- Başlangıçtaki depolar --" | tee -a "$log_file"
  pisi lr 2>&1 | tee -a "$log_file" || true
  update-ca-certificates 2>&1 | tee -a "$log_file" || true
  add_repo_if_missing core https://github.com/pisilinux/core/raw/master/pisi-index.xml.xz
  add_repo_if_missing main https://github.com/pisilinux/main/raw/master/pisi-index.xml.xz
  echo "-- Son depolar --" | tee -a "$log_file"
  pisi lr 2>&1 | tee -a "$log_file" || true
}

update_repos() {
  log_phase "Depolar güncelleniyor (pisi ur)"
  pisi ur 2>&1 | tee -a "$log_file"
  return "${PIPESTATUS[0]}"
}

upgrade_system() {
  if [ "$full_upgrade" = "true" ]; then
    log_phase "Tam sistem yükseltmesi çalışıyor (pisi up)"
    pisi up -dvsy --ignore-safety 2>&1 | tee -a "$log_file"
  else
    log_phase "Sistem yükseltmesi atlandı (full_upgrade=false)"
  fi
}

build_package() {
  log_phase "Paket derleniyor: $recipe_path"
  cd "$output_dir" || return 1
  pisi bi -dy --ignore-safety "/git/$recipe_path/pspec.xml" 2>&1 | tee -a "$log_file"
  return "${PIPESTATUS[0]}"
}

list_packages() {
  log_phase "Üretilen paketler"
  find "$output_dir" -maxdepth 1 -name '*.pisi' -printf '%s\t%f\n' | sort -k2 > "$output_dir/packages.txt"
  cat "$output_dir/packages.txt" | tee -a "$log_file"
}

start_dbus
configure_repos
update_repos || echo "pisi ur başarısız oldu, derleme yine de deneniyor" | tee -a "$log_file"
upgrade_system || echo "Yükseltme başarısız oldu, derleme yine de deneniyor" | tee -a "$log_file"

build_package
build_status=$?

echo "$build_status" > "$output_dir/status.txt"
list_packages
log_phase "Derleme çıkış kodu: $build_status"
exit "$build_status"
