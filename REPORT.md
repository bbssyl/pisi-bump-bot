# Pisi Linux contrib güncellik raporu

Kaynak: https://git.pisilinux.org/Pisilinux/contrib (pspec.xml dosyaları, commit `973adc1`)

## Özet

- Toplam paket: 141
- Eski: 29
- Güncel: 8
- Desteklenmiyor: 98
- Karşılaştırılamadı: 0
- Hata: 6
- Index'te olmayan: 39
- Index sürüm farkı: 44

## Eski paketler

| Paket | Pspec | Mevcut | Yeni | Kaynak | Aday arşiv URL | Hazır pspec | Derleme |
| --- | --- | --- | --- | --- | --- | --- | --- |
| atari800 | `game/emulator/atari800/pspec.xml` | 5.2.0 | ATARI800_7_2_1 | [atari800/atari800](https://github.com/atari800/atari800/releases/tag/ATARI800_7_2_1) | `https://github.com/atari800/atari800/releases/download/ATARI800_7_2_1/atari800-7.2.1-src.tgz` | [pspec.diff](hazir/game/emulator/atari800/pspec.diff) | [✅](https://github.com/bbssyl/pisi-bump-bot/actions/runs/37625455360) |
| brave-browser | `network/browser/brave/pspec.xml` | 1.93.129 | v1.97.56 | [brave/brave-browser](https://github.com/brave/brave-browser/releases/tag/v1.97.56) | `https://github.com/brave/brave-browser/releases/download/v1.97.56/brave-browser-1.97.56-linux-amd64.zip` | [pspec.diff](hazir/network/browser/brave/pspec.diff) | [✅](https://github.com/bbssyl/pisi-bump-bot/actions/runs/37721855164) |
| brave-browser-nightly | `network/browser/brave-nightly/pspec.xml` | 1.95.29 | v1.97.56 | [brave/brave-browser](https://github.com/brave/brave-browser/releases/tag/v1.97.56) | - | - | otomatik hazırlanamadı: uygun dosya bulunamadı (adaylar: brave-browser-1.97.56-1.aarch64.rpm, brave-browser-1.97.56-1.x86_64.rpm, brave-browser-1.97.56-linux-amd64.zip, brave-browser-1.97.56-linux-arm64.zip, Brave-Browser-arm64.dmg) |
| Dune2000 | `game/strategy/Dune2000/pspec.xml` | 20210321 | release-20250330 | [OpenRA/OpenRA](https://github.com/OpenRA/OpenRA/releases/tag/release-20250330) | `https://github.com/OpenRA/OpenRA/releases/download/release-20250330/OpenRA-Dune-2000-x86_64.AppImage` | [pspec.diff](hazir/game/strategy/Dune2000/pspec.diff) | [✅](https://github.com/bbssyl/pisi-bump-bot/actions/runs/37625455360) |
| etcher | `util/admin/etcher/pspec.xml` | 1.7.3 | v2.1.7 | [balena-io/etcher](https://github.com/balena-io/etcher/releases/tag/v2.1.7) | - | - | otomatik hazırlanamadı: uygun dosya bulunamadı (adaylar: balena-etcher-2.1.7-1.x86_64.rpm, balena-etcher_2.1.7_amd64.deb, balenaEtcher-2.1.7-arm64.dmg, balenaEtcher-2.1.7-x64.dmg, balenaEtcher-2.1.7.Setup.exe) |
| ferdium | `network/chat/ferdium/pspec.xml` | 7.1.1 | v7.2.3 | [ferdium/ferdium-app](https://github.com/ferdium/ferdium-app/releases/tag/v7.2.3) | `https://github.com/ferdium/ferdium-app/releases/download/v7.2.3/Ferdium-linux-7.2.3-amd64.deb` | [pspec.diff](hazir/network/chat/ferdium/pspec.diff) | [✅](https://github.com/bbssyl/pisi-bump-bot/actions/runs/37630041914) |
| FreeCAD | `multimedia/graphics/freecad/pspec.xml` | 1.0.1 | 1.1.4 | [FreeCAD/FreeCAD](https://github.com/FreeCAD/FreeCAD/releases/tag/1.1.4) | `https://github.com/FreeCAD/FreeCAD/releases/download/1.1.4/FreeCAD_1.1.4-Linux-x86_64-py311.AppImage` | [pspec.diff](hazir/multimedia/graphics/freecad/pspec.diff) | [❌](https://github.com/bbssyl/pisi-bump-bot/actions/runs/37782670830) |
| fritzing | `science/electronics/fritzing/pspec.xml` | 0.9.4 | CD-625 | [fritzing/fritzing-app](https://github.com/fritzing/fritzing-app/releases/tag/CD-625) | `https://github.com/fritzing/fritzing-app/releases/download/CD-625/fritzing-a1ffcea08814801903b1a9515b18cf97067968ae-master-498.xenial.linux.AMD64.tar.bz2` | - | otomatik hazırlanamadı: indirme başarısız (HTTP 404) |
| github-desktop | `network/misc/github-desktop/pspec.xml` | 3.4.9 | release-3.4.13-linux1 | [shiftkey/desktop](https://github.com/shiftkey/desktop/releases/tag/release-3.4.13-linux1) | `https://github.com/shiftkey/desktop/releases/download/release-3.4.13-linux1/GitHubDesktop-linux-amd64-3.4.13-linux1.deb` | [pspec.diff](hazir/network/misc/github-desktop/pspec.diff) | [✅](https://github.com/bbssyl/pisi-bump-bot/actions/runs/37630041914) |
| gnofract4d | `gnofract4d/pspec.xml` | 3.14.1 | v4.4 | [edyoung/gnofract4d](https://github.com/edyoung/gnofract4d/releases/tag/v4.4) | `https://github.com/edyoung/gnofract4d/archive/v4.4.tar.gz` | [pspec.diff](hazir/gnofract4d/pspec.diff) | [❌](https://github.com/bbssyl/pisi-bump-bot/actions/runs/37625455360) |
| iptvnator | `multimedia/tv/iptvnator/pspec.xml` | 0.15.0 | v0.24.0 | [4gray/iptvnator](https://github.com/4gray/iptvnator/releases/tag/v0.24.0) | `https://github.com/4gray/iptvnator/releases/download/v0.24.0/iptvnator-0.24.0-linux-amd64.deb` | [pspec.diff](hazir/multimedia/tv/iptvnator/pspec.diff) | [❌](https://github.com/bbssyl/pisi-bump-bot/actions/runs/37782670830) |
| koodo-reader | `office/koodo-reader/pspec.xml` | 1.4.5 | v2.4.6 | [troyeguo/koodo-reader](https://github.com/troyeguo/koodo-reader/releases/tag/v2.4.6) | `https://github.com/troyeguo/koodo-reader/releases/download/v2.4.6/Koodo-Reader-2.4.6-x86_64.AppImage` | [pspec.diff](hazir/office/koodo-reader/pspec.diff) | [❌](https://github.com/bbssyl/pisi-bump-bot/actions/runs/37630041914) |
| mailspring | `network/mail/mailspring/pspec.xml` | 1.19.0 | 1.26.0 | [Foundry376/Mailspring](https://github.com/Foundry376/Mailspring/releases/tag/1.26.0) | `https://github.com/Foundry376/Mailspring/releases/download/1.26.0/mailspring-1.26.0-amd64.deb` | [pspec.diff](hazir/network/mail/mailspring/pspec.diff) | [✅](https://github.com/bbssyl/pisi-bump-bot/actions/runs/37630041914) |
| Mattermost | `network/chat/mattermost/pspec.xml` | 6.1.0 | v6.3.0 | [mattermost/desktop](https://github.com/mattermost/desktop/releases/tag/v6.3.0) | `https://github.com/mattermost/desktop/releases/download/v6.3.0/mattermost-desktop-6.3.0-linux-x64.tar.gz` | [pspec.diff](hazir/network/chat/mattermost/pspec.diff) | [✅](https://github.com/bbssyl/pisi-bump-bot/actions/runs/37630041914) |
| obsidian | `editor/obsidian/pspec.xml` | 1.5.3 | v1.14.4 | [obsidianmd/obsidian-releases](https://github.com/obsidianmd/obsidian-releases/releases/tag/v1.14.4) | `https://github.com/obsidianmd/obsidian-releases/releases/download/v1.14.4/obsidian_1.14.4_amd64.deb` | [pspec.diff](hazir/editor/obsidian/pspec.diff) | [✅](https://github.com/bbssyl/pisi-bump-bot/actions/runs/37625455360) |
| onlyoffice | `office/onlyoffice/pspec.xml` | 9.3.1 | v9.4.0 | [ONLYOFFICE/DesktopEditors](https://github.com/ONLYOFFICE/DesktopEditors/releases/tag/v9.4.0) | `https://github.com/ONLYOFFICE/DesktopEditors/releases/download/v9.4.0/onlyoffice-desktopeditors-x64.tar.xz` | [pspec.diff](hazir/office/onlyoffice/pspec.diff) | [✅](https://github.com/bbssyl/pisi-bump-bot/actions/runs/37630041914) |
| openboardview | `science/electronics/openboardview/pspec.xml` | 9.0.3 | 10.0.0 | [OpenBoardView/OpenBoardView](https://github.com/OpenBoardView/OpenBoardView/releases/tag/10.0.0) | `https://github.com/OpenBoardView/OpenBoardView/releases/download/10.0.0/openboardview_10.0.0-1_amd64.deb` | [pspec.diff](hazir/science/electronics/openboardview/pspec.diff) | [❌](https://github.com/bbssyl/pisi-bump-bot/actions/runs/37721855164) |
| pencil2d | `multimedia/graphics/pencil2d/pspec.xml` | 0.6.6 | v0.7.2 | [pencil2d/pencil](https://github.com/pencil2d/pencil/releases/tag/v0.7.2) | `https://github.com/pencil2d/pencil/releases/download/v0.7.2/pencil2d-linux-amd64-v0.7.2.AppImage` | [pspec.diff](hazir/multimedia/graphics/pencil2d/pspec.diff) | [❌](https://github.com/bbssyl/pisi-bump-bot/actions/runs/37782670830) |
| Pot-Desktop | `pot-desktop/pspec.xml` | 2.7.4 | 3.0.7 | [pot-app/pot-desktop](https://github.com/pot-app/pot-desktop/releases/tag/3.0.7) | `https://github.com/pot-app/pot-desktop/releases/download/3.0.7/pot_3.0.7_amd64.AppImage` | [pspec.diff](hazir/pot-desktop/pspec.diff) | [❌](https://github.com/bbssyl/pisi-bump-bot/actions/runs/37630041914) |
| RedAlert | `game/strategy/RedAlert/pspec.xml` | 20210321 | release-20250330 | [OpenRA/OpenRA](https://github.com/OpenRA/OpenRA/releases/tag/release-20250330) | `https://github.com/OpenRA/OpenRA/releases/download/release-20250330/OpenRA-Red-Alert-x86_64.AppImage` | [pspec.diff](hazir/game/strategy/RedAlert/pspec.diff) | [✅](https://github.com/bbssyl/pisi-bump-bot/actions/runs/37625455360) |
| SynfigStudio | `multimedia/graphics/synfig/pspec.xml` | 1.5.1 | v1.5.5 | [synfig/synfig](https://github.com/synfig/synfig/releases/tag/v1.5.5) | `https://github.com/synfig/synfig/releases/download/v1.5.5/SynfigStudio-1.5.5-2026.03.15-linux64-79bf7.appimage` | [pspec.diff](hazir/multimedia/graphics/synfig/pspec.diff) | [❌](https://github.com/bbssyl/pisi-bump-bot/actions/runs/37782670830) |
| tesseract | `tesseract/pspec.xml` | 4.0.0 | 5.5.3 | [tesseract-ocr/tesseract](https://github.com/tesseract-ocr/tesseract/releases/tag/5.5.3) | `https://github.com/tesseract-ocr/tesseract/archive/5.5.3.tar.gz` | [pspec.diff](hazir/tesseract/pspec.diff) | [❌](https://github.com/bbssyl/pisi-bump-bot/actions/runs/37721855164) |
| toggldesktop | `toggldesktop/pspec.xml` | 7.4.1023 | v7.636 | [toggl/toggldesktop](https://github.com/toggl/toggldesktop/releases/tag/v7.636) | `https://github.com/toggl/toggldesktop/archive/v7.636.tar.gz` | [pspec.diff](hazir/toggldesktop/pspec.diff) | [❌](https://github.com/bbssyl/pisi-bump-bot/actions/runs/37721855164) |
| translate-shell | `translate-shell/pspec.xml` | 0.9.6.12 | v0.9.7.1 | [soimort/translate-shell](https://github.com/soimort/translate-shell/releases/tag/v0.9.7.1) | `https://codeload.github.com/soimort/translate-shell/tar.gz/refs/tags/v0.9.7.1` | [pspec.diff](hazir/translate-shell/pspec.diff) | [✅](https://github.com/bbssyl/pisi-bump-bot/actions/runs/37721855164) |
| ventoy | `util/admin/ventoy/pspec.xml` | 1.1.12 | v1.1.17 | [ventoy/Ventoy](https://github.com/ventoy/Ventoy/releases/tag/v1.1.17) | `https://github.com/ventoy/Ventoy/releases/download/v1.1.17/ventoy-1.1.17-linux.tar.gz` | [pspec.diff](hazir/util/admin/ventoy/pspec.diff) | [✅](https://github.com/bbssyl/pisi-bump-bot/actions/runs/37721855164) |
| VSCodium | `development/VSCodium/pspec.xml` | 1.112.01907 | 1.135.06055 | [VSCodium/vscodium](https://github.com/VSCodium/vscodium/releases/tag/1.135.06055) | `https://github.com/VSCodium/vscodium/releases/download/1.135.06055/VSCodium-linux-x64-1.135.06055.tar.gz` | [pspec.diff](hazir/development/VSCodium/pspec.diff) | [✅](https://github.com/bbssyl/pisi-bump-bot/actions/runs/37625455360) |
| webcamoid-ffmpeg | `webcamoid-ffmpeg/pspec.xml` | 7.2.1 | 9.4.0 | [webcamoid/webcamoid](https://github.com/webcamoid/webcamoid/releases/tag/9.4.0) | `https://github.com/webcamoid/webcamoid/archive/9.4.0.tar.gz` | [pspec.diff](hazir/webcamoid-ffmpeg/pspec.diff) | [❌](https://github.com/bbssyl/pisi-bump-bot/actions/runs/37721855164) |
| wordpress-desktop | `wordpress-desktop/pspec.xml` | 8.0.2 | v8.2.4 | [Automattic/wp-desktop](https://github.com/Automattic/wp-desktop/releases/tag/v8.2.4) | `https://github.com/Automattic/wp-desktop/releases/download/v8.2.4/wordpress.com-linux-x64-8.2.4.tar.gz` | [pspec.diff](hazir/wordpress-desktop/pspec.diff) | [✅](https://github.com/bbssyl/pisi-bump-bot/actions/runs/37782670830) |
| zen-browser | `network/browser/zen-browser/pspec.xml` | 1.21.10b | 1.23.1b | [zen-browser/desktop](https://github.com/zen-browser/desktop/releases/tag/1.23.1b) | `https://github.com/zen-browser/desktop/releases/download/1.23.1b/zen-x86_64.AppImage` | [pspec.diff](hazir/network/browser/zen-browser/pspec.diff) | [✅](https://github.com/bbssyl/pisi-bump-bot/actions/runs/37630041914) |

<details>
<summary>Güncel paketler (8)</summary>

| Paket | Pspec | Mevcut | Yeni | Kaynak | Ayrıntı |
| --- | --- | --- | --- | --- | --- |
| chat-gpt | `chat-gpt/pspec.xml` | 1.1.0 | v1.1.0 | lencx/ChatGPT | - |
| figma-linux | `development/figma-linux/pspec.xml` | 0.11.5 | v0.11.5 | Figma-Linux/figma-linux | - |
| franz | `network/chat/franz/pspec.xml` | 5.11.0 | v5.11.0 | meetfranz/franz | - |
| mBlock | `science/electronics/mBlock/pspec.xml` | 4.0.0 | V4.0.0-Linux | Makeblock-official/mBlock | - |
| meb-certs | `hangar/meb-certs/pspec.xml` | 1.0 | v1.0 | kurtbahartr/meb-certs-pisi | - |
| ramme | `ramme/pspec.xml` | 3.2.5 | v3.2.5 | terkelg/ramme | - |
| scratch-desktop | `programming/language/scratch-desktop/pspec.xml` | 3.3.0 | 3.3.0 | redshaderobotics/scratch3.0-linux | - |
| ventoy | `hardware/disk/ventoy/pspec.xml` | 1.1.17 | v1.1.17 | ventoy/Ventoy | - |

</details>

<details>
<summary>Desteklenmeyen paketler (98)</summary>

| Paket | Pspec | Mevcut | Yeni | Kaynak | Ayrıntı |
| --- | --- | --- | --- | --- | --- |
| acs-unified-driver | `hangar/akiskart/acs-unified-driver/pspec.xml` | 1.1.8 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| adalet-eimza-tray | `hangar/adalet-eimza-tray/pspec.xml` | 1.0.11 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| akiskart | `hangar/akiskart/akiskart/pspec.xml` | 6.7.6 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| android-studio | `development/android-studio/pspec.xml` | 2025.1.2.11 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| antigravity | `development/antigravity/pspec.xml` | 2.0.1 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| Antigravity-ide | `development/antigravity-ide/pspec.xml` | 2.5.5 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| anydesk | `network/misc/anydesk/pspec.xml` | 8.0.2 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| Apache Netbeans | `development/apache-netbeans/pspec.xml` | 26 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| apfs-fuse | `hangar/akiskart/apfs-fuse/pspec.xml` | 0.1.0 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| arduino | `science/electronics/arduino/pspec.xml` | 2.3.3 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| arksigner | `hangar/arksigner/pspec.xml` | 2.3.13 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| binance | `binance/pspec.xml` | 1.47.0 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| brother-1210w | `hardware/printer-and-scanner/brother-1210w/pspec.xml` | 3.0.1 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| brother-dcpt510w | `hardware/printer-and-scanner/brother-dcpt510w-printer/pspec.xml` | 1.0.1 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| brscan2 | `hardware/printer-and-scanner/Brother-brscan2/pspec.xml` | 0.2.5 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| brscan3 | `hardware/printer-and-scanner/Brother-brscan3/pspec.xml` | 0.2.11 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| brscan4 | `hardware/printer-and-scanner/Brother-brscan4/pspec.xml` | 0.4.2 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| chromium-widevine | `network/browser/chromium-widevine/pspec.xml` | 4.10.3050.0 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| cnijfilter-common | `hardware/printer-and-scanner/cnijfilter-common/pspec.xml` | 3.40 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| Cursor | `development/cursor/pspec.xml` | 3.20.17 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| dcp135ccupswrapper-1.0.1 | `hardware/printer-and-scanner/Brother-DCP135c-cupswrapper/pspec.xml` | 1.0.1 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| dcp135clpr-1.0.1 | `hardware/printer-and-scanner/Brother-DCP135c-lpr/pspec.xml` | 1.0.1 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| devin-desktop | `development/devin-desktop/pspec.xml` | 3.10.23 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| discord | `network/chat/discord/pspec.xml` | 0.0.118 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| dropbox-client | `network/misc/dropbox-client/pspec.xml` | 246.4.3513 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| eagle | `science/electronics/eagle/pspec.xml` | 9.6.2 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| eclipse | `development/eclipse/pspec.xml` | 2025.06 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| firefox | `network/browser/firefox-bin/pspec.xml` | 153.0 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| fpc | `development/lazarus/fpc/pspec.xml` | 3.0.4 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| fpc | `development/lazarusDEB/fpc/pspec.xml` | 3.0.4 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| fpc-src | `development/lazarus/fpc-src/pspec.xml` | 3.0.4 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| fpc-src | `development/lazarusDEB/fpc-src/pspec.xml` | 3.0.4 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| freedownloadmanager | `network/misc/freedownloadmanager/pspec.xml` | 6.33.2.6656 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| Front-Panel-Design | `multimedia/graphics/front_panel_designer/pspec.xml` | 6.3.6 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| gitbook-editor | `editor/gitbook-editor/pspec.xml` | 6.2.0 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| gitkraken | `network/misc/gitkraken/pspec.xml` | 11.10.0 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| google-chrome | `network/browser/google-chrome/pspec.xml` | 151.0.7922.71 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| google-chrome-dev | `network/browser/google-chrome-dev/pspec.xml` | 153.0.7979.3 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| google-earth | `google-earth/pspec.xml` | 7.3.6.9345 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| google-talkplugin | `google-talkplugin/pspec.xml` | 5.41.0 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| gradle | `gradle/pspec.xml` | 4.1 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| i-nex | `i-nex/pspec.xml` | 7.6.1 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| insync | `network/misc/insync/pspec.xml` | 3.9.8.60034 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| JEdit | `jedit/pspec.xml` | 5.5.0 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| jopdf | `editor/jopdf/pspec.xml` | 2.2.0 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| lazarus | `development/lazarus/lazaruz/pspec.xml` | 4.6 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| lazarus | `development/lazarusDEB/lazaruz/pspec.xml` | 4.6 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| leptonica | `leptonica/pspec.xml` | 1.77.0 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| luckybackup | `luckybackup/pspec.xml` | 0.4.9 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| manicminer | `game/arcade/manicminer/pspec.xml` | 1.7a | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| master-pdf-editor | `editor/master-pdf-editor/pspec.xml` | 5.8.52 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| Microsoft-Edge | `network/browser/microsoft-edge/pspec.xml` | 151.0.4129.59 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| netflix-desktop | `multimedia/tv/netflix-desktop/pspec.xml` | 1.0.3 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| ngrok | `network/misc/ngrok/pspec.xml` | 3.37.3 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| numlockx | `numlockx/pspec.xml` | 1.2 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| ocenaudio | `multimedia/sound/oceanaudio/pspec.xml` | 3.18.2 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| opera | `network/browser/opera/pspec.xml` | 133.0.5932.85 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| opera-beta | `network/browser/opera-beta/pspec.xml` | 126.0.5750.30 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| opera-developer | `network/browser/opera-developer/pspec.xml` | 135.0.5966.0 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| packet-tracer | `network/misc/packet-tracer/pspec.xml` | 8.20 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| Pencil | `multimedia/graphics/pencil/pspec.xml` | 3.1.1 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| phpstorm | `development/phpstorm/pspec.xml` | 2025.2 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| pisi-linux-imagewriter | `pisi-linux-imagewriter/pspec.xml` | 2.6.2 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| plex-media-server | `multimedia/server/plex-media-server/pspec.xml` | 1.23.1.4602 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| poco | `poco/pspec.xml` | 1.9.4 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| PrinceofPersia | `game/arcade/PrinceofPersia/pspec.xml` | 1.23 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| ProjectLibre | `ProjectLibre/pspec.xml` | 1.9.1 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| pycharm-community | `development/pycharm-community/pspec.xml` | 2025.2 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| python-gtksourceview | `python-gtksourceview/pspec.xml` | 2.10.0 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| rpm2targz | `rpm2targz/pspec.xml` | 9.0.0.5g | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| scrivener | `scrivener/pspec.xml` | 1.9.0.1 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| spotify | `multimedia/sound/spotify/pspec.xml` | 1.2.63.394 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| steam | `network/misc/steam/pspec.xml` | 1.0.0.85 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| studio-3t | `development/studio-3T/pspec.xml` | 2026.3.0 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| sublime-merge | `development/sublime-merge/pspec.xml` | 2039 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| sublime-text | `development/sublime-text/pspec.xml` | 4200 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| teamspeak3 | `teamspeak3/pspec.xml` | 3.1.3 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| teamviewer | `network/misc/teamviewer/pspec.xml` | 15.76.5 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| teamviewer-fonts | `teamviewer-font/pspec.xml` | 0.1 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| uhapsigner | `hangar/uhapsigner/pspec.xml` | 2.0.5 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| utopia | `network/chat/utopia/pspec.xml` | 1.3.1262 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| uyap | `hangar/uyap/pspec.xml` | 5.4.20 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| Valentina Studio | `valentina_studio/pspec.xml` | 16.0.1 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| vdrift | `game/sports/vdrift/pspec.xml` | 20141020 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| viber | `network/chat/viber/pspec.xml` | 27.3.0.2 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| vivaldi-browser | `network/browser/vivaldi/pspec.xml` | 8.1.4087.61 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| vs-code-editor | `vs-code-editor/pspec.xml` | 1.2.1 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| VSCode | `development/VScode/pspec.xml` | 1.132 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| weasis | `science/weasis/pspec.xml` | 3.8.0 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| webex | `network/chat/webex/pspec.xml` | 46.2.1.34187 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| wps-office | `office/wps-office/pspec.xml` | 11.1.0.10702 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| XnViewMP | `XnViewMP/pspec.xml` | 0.98.3 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| Yandex Disk cli | `network/misc/yandeskdisk_cli/pspec.xml` | 0.1.6.1080 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| yandex-disk-indicator | `network/misc/yandex-disk-indicator/pspec.xml` | 1.11.0 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| youtube-to-MP3 | `multimedia/sound/youtube-to-MP3/pspec.xml` | 3.9.9.48 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| zenity | `zenity/pspec.xml` | 3.20.0 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| zoom-workplace | `network/chat/zoom-workplace/pspec.xml` | 7.0.0.1666 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |
| zotero | `zotero/pspec.xml` | 5.0.96.3 | - | - | GitHub dışı veya desteklenmeyen arşiv URL'si |

</details>

<details>
<summary>Hata veren paketler (6)</summary>

| Paket | Pspec | Mevcut | Yeni | Kaynak | Ayrıntı |
| --- | --- | --- | --- | --- | --- |
| belgenet | `hangar/belgenet/pspec.xml` | - | - | - | pspec.xml okunamadı |
| jelvis | `jelvis/pspec.xml` | 1.0.2 | - | kiahamedi/JELVIS-git | release veya tag yok |
| mp | `mp/pspec.xml` | - | - | - | pspec.xml okunamadı |
| odin4-cli | `odin4-cli/pspec.xml` | 1.2.1 | - | Adrilaw/OdinV4 | depo bulunamadı |
| qt5-fsarchiver | `qt5-fsarchiver/pspec.xml` | 0.8.0 | - | DieterBaum/qt5-fsarchiver | depo bulunamadı |
| tailscale-static | `tailscale-static/pspec.xml` | - | - | - | pspec.xml okunamadı |

</details>

<details>
<summary>Index tutarsızlıkları (83)</summary>

Index'te olmayan paketler (39):

- `chat-gpt/pspec.xml`
- `development/VSCodium/pspec.xml`
- `development/antigravity-ide/pspec.xml`
- `development/antigravity/pspec.xml`
- `development/cursor/pspec.xml`
- `development/devin-desktop/pspec.xml`
- `development/lazarus/fpc-src/pspec.xml`
- `development/lazarus/fpc/pspec.xml`
- `development/lazarus/lazaruz/pspec.xml`
- `development/lazarusDEB/fpc-src/pspec.xml`
- `development/lazarusDEB/fpc/pspec.xml`
- `development/lazarusDEB/lazaruz/pspec.xml`
- `development/studio-3T/pspec.xml`
- `editor/jopdf/pspec.xml`
- `editor/obsidian/pspec.xml`
- `game/arcade/PrinceofPersia/pspec.xml`
- `game/arcade/manicminer/pspec.xml`
- `game/emulator/atari800/pspec.xml`
- `hangar/adalet-eimza-tray/pspec.xml`
- `hangar/akiskart/acs-unified-driver/pspec.xml`
- `hangar/akiskart/akiskart/pspec.xml`
- `hangar/akiskart/apfs-fuse/pspec.xml`
- `hangar/arksigner/pspec.xml`
- `hangar/meb-certs/pspec.xml`
- `hangar/uhapsigner/pspec.xml`
- `hangar/uyap/pspec.xml`
- `hardware/disk/ventoy/pspec.xml`
- `hardware/printer-and-scanner/brother-dcpt510w-printer/pspec.xml`
- `multimedia/graphics/front_panel_designer/pspec.xml`
- `multimedia/sound/oceanaudio/pspec.xml`
- `multimedia/tv/iptvnator/pspec.xml`
- `network/browser/brave-nightly/pspec.xml`
- `network/browser/zen-browser/pspec.xml`
- `network/chat/ferdium/pspec.xml`
- `network/chat/zoom-workplace/pspec.xml`
- `network/misc/freedownloadmanager/pspec.xml`
- `network/misc/packet-tracer/pspec.xml`
- `odin4-cli/pspec.xml`
- `pot-desktop/pspec.xml`

Pspec sürümü index sürümünden farklı olanlar (44):

| Paket | Pspec | Pspec sürümü | Index sürümü |
| --- | --- | --- | --- |
| binance | `binance/pspec.xml` | 1.47.0 | 1.16.2 |
| VSCode | `development/VScode/pspec.xml` | 1.132 | 1.71 |
| android-studio | `development/android-studio/pspec.xml` | 2025.1.2.11 | 202.7486908 |
| Apache Netbeans | `development/apache-netbeans/pspec.xml` | 26 | 12.3 |
| eclipse | `development/eclipse/pspec.xml` | 2025.06 | 2021.3 |
| figma-linux | `development/figma-linux/pspec.xml` | 0.11.5 | 0.9.6 |
| phpstorm | `development/phpstorm/pspec.xml` | 2025.2 | 2021.2.3 |
| pycharm-community | `development/pycharm-community/pspec.xml` | 2025.2 | 2021.3.2 |
| sublime-text | `development/sublime-text/pspec.xml` | 4200 | 4121 |
| google-earth | `google-earth/pspec.xml` | 7.3.6.9345 | 7.3.4.8642 |
| FreeCAD | `multimedia/graphics/freecad/pspec.xml` | 1.0.1 | 0.19.3 |
| Pencil | `multimedia/graphics/pencil/pspec.xml` | 3.1.1 | 3.1.0 |
| spotify | `multimedia/sound/spotify/pspec.xml` | 1.2.63.394 | 1.1.84.716 |
| brave-browser | `network/browser/brave/pspec.xml` | 1.93.129 | 1.43.42 |
| chromium-widevine | `network/browser/chromium-widevine/pspec.xml` | 4.10.3050.0 | 4.10.1679.0 |
| firefox | `network/browser/firefox-bin/pspec.xml` | 153.0 | 59.0.0 |
| google-chrome-dev | `network/browser/google-chrome-dev/pspec.xml` | 153.0.7979.3 | 88.0.4292.2 |
| google-chrome | `network/browser/google-chrome/pspec.xml` | 151.0.7922.71 | 103.0.5060.134 |
| Microsoft-Edge | `network/browser/microsoft-edge/pspec.xml` | 151.0.4129.59 | 103.0.1264.71 |
| opera-beta | `network/browser/opera-beta/pspec.xml` | 126.0.5750.30 | 72.0.3815.133 |
| opera-developer | `network/browser/opera-developer/pspec.xml` | 135.0.5966.0 | 73.0.3847.0 |
| opera | `network/browser/opera/pspec.xml` | 133.0.5932.85 | 89.0.4447.51 |
| vivaldi-browser | `network/browser/vivaldi/pspec.xml` | 8.1.4087.61 | 5.3.2679.70 |
| discord | `network/chat/discord/pspec.xml` | 0.0.118 | 0.0.19 |
| franz | `network/chat/franz/pspec.xml` | 5.11.0 | 5.7.0 |
| Mattermost | `network/chat/mattermost/pspec.xml` | 6.1.0 | 5.0.2 |
| utopia | `network/chat/utopia/pspec.xml` | 1.3.1262 | 1.1 |
| viber | `network/chat/viber/pspec.xml` | 27.3.0.2 | 13.3.1.22 |
| webex | `network/chat/webex/pspec.xml` | 46.2.1.34187 | 42.4.0.21893 |
| mailspring | `network/mail/mailspring/pspec.xml` | 1.19.0 | 1.9.2 |
| anydesk | `network/misc/anydesk/pspec.xml` | 8.0.2 | 6.1.1 |
| dropbox-client | `network/misc/dropbox-client/pspec.xml` | 246.4.3513 | 80.4.127 |
| github-desktop | `network/misc/github-desktop/pspec.xml` | 3.4.9 | 2.9.12 |
| gitkraken | `network/misc/gitkraken/pspec.xml` | 11.10.0 | 8.3.3 |
| insync | `network/misc/insync/pspec.xml` | 3.9.8.60034 | 3.7.5.50350 |
| ngrok | `network/misc/ngrok/pspec.xml` | 3.37.3 | 2.3.40 |
| steam | `network/misc/steam/pspec.xml` | 1.0.0.85 | 1.0.0.74 |
| teamviewer | `network/misc/teamviewer/pspec.xml` | 15.76.5 | 15.16.8 |
| Yandex Disk cli | `network/misc/yandeskdisk_cli/pspec.xml` | 0.1.6.1080 | 0.1.5.1039 |
| onlyoffice | `office/onlyoffice/pspec.xml` | 9.3.1 | 7.0.0 |
| arduino | `science/electronics/arduino/pspec.xml` | 2.3.3 | 1.8.12 |
| ventoy | `util/admin/ventoy/pspec.xml` | 1.1.12 | 1.0.79 |
| Valentina Studio | `valentina_studio/pspec.xml` | 16.0.1 | 8.7.2 |
| wordpress-desktop | `wordpress-desktop/pspec.xml` | 8.0.2 | 4.6.0 |

</details>
