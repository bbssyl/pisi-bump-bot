# pisi-bump-bot

Pisi Linux `contrib` deposundaki paketlerin GitHub üzerindeki yeni sürümlerini her gün kontrol eden ve sonucu raporlayan bir bot.

Bot yalnızca GitHub Actions üzerinde çalışır (günlük zamanlanmış görev). Her çalışmada `contrib` deposunu salt okunur ve sığ (`--depth 1`) olarak klonlar, `0oldpackage/` dışındaki tüm `pspec.xml` dosyalarını okur ve gerçek kaynak olarak bunları kullanır. Mevcut sürüm, `History` içindeki en yüksek `release` numaralı `Update` kaydından alınır. Her paketin arşiv URL'sinden GitHub deposu çıkarılır ve en son release (yoksa en son tag) ile karşılaştırılır.

Aynı isimli paketler (ör. iki farklı dizindeki `ventoy`) ayrı satırlar olarak, `pspec.xml` yolu ile birlikte gösterilir. Raporun başlığında incelenen contrib commit'i yer alır.

## Yalnızca rapor verir

Bot hiçbir zaman:

- Pisi depolarına yazmaz veya push yapmaz.
- Pisi depolarında PR veya issue açmaz.
- Başka bir yerde PR açmaz.
- Workflow'un varsayılan `GITHUB_TOKEN` değeri dışında bir token kullanmaz.
- Telegram gibi başka bir kanala bildirim göndermez.

Yalnızca kendi deposuna yazar: `REPORT.md`, `report.json`, `hazir/`, `state/builds.json` ve tek bir pano issue'su.

## Sonuçları nerede görebilirsiniz

- Actions: ilgili çalışmanın adım özeti (Step Summary).
- Pano issue'su: `rapor-panosu` etiketli "Pisi contrib güncellik panosu". Yeni çıkan güncellemeler bu issue'ya yorum olarak eklenir.
- `REPORT.md` ve `report.json`: yalnızca içerik değiştiğinde commit edilir.

Rapor durumları: `eski`, `guncel`, `desteklenmiyor`, `karsilastirilamadi`, `hata`. Raporda ayrıca bilgi amaçlı "Index tutarsızlıkları" bölümü bulunur. Klonda `pisi-index.xml.xz` (veya `pisi-index.xml`) varsa, index'te olmayan paketler ve `pspec.xml` sürümü index sürümünden farklı olan paketler listelenir. Sürüm karşılaştırması index'e değil `pspec.xml` dosyalarına dayanır.

## Hazır pspec ve deneme derlemesi

Workflow üç işten oluşur: `report` (rapor + hazırlık), `build` (matrix, en fazla 5 paralel) ve `publish` (commit, pano). Her çalışmada en fazla 10 `eski` GitHub paketi için aday arşiv indirilir, sha1 hesaplanır ve yalnızca `Archive` + yeni `History` kaydı değiştirilerek `hazir/<paket yolu>/pspec.xml` ve `pspec.diff` üretilir (bağımlılıklara dokunulmaz). Arşiv inmezse paket "otomatik hazırlanamadı: <neden>" olarak işaretlenir ve aynı sürüm için tekrar denenmez.

Hazırlanan pspec, `pisilinux/chroot` kapsayıcısında derlenir. Derleme günlüğü ve `.pisi` dosyaları (1 GB üstü hariç) 14 gün saklanan artifact olarak yüklenir. Derlenen paketler yalnızca denemedir. Durum `state/builds.json` içinde tutulur; aynı yol + sürüm bir daha derlenmez. Paket artık `eski` değilse `hazir/` ve durum kaydı silinir.

## Elle çalıştırma

GitHub'da depoda Actions sekmesine gidin, "Güncellik raporu" workflow'unu seçin ve Run workflow düğmesine basın.

## Zamanlama

Her gün 03:00 UTC (Türkiye saatiyle 06:00). Son commit 50 günden eskiyse, zamanlanmış görevlerin devre dışı kalmaması için boş bir `chore: keepalive` commit'i atılır.

## Sınırlamalar

- Yalnızca GitHub kaynaklı arşivler kontrol edilir. Diğer URL'ler (GitLab, SourceForge, doğrudan indirme vb.), dal anlık görüntüleri ve `raw` dosya bağlantıları `desteklenmiyor` olarak listelenir.
- Yalnızca `contrib` deposu desteklenir.
- Okunamayan (bozuk XML) `pspec.xml` dosyaları `hata` olarak listelenir.
- Index güncel olmayabilir. Bu yüzden sürümler index'ten değil `pspec.xml` dosyalarından okunur.
- Sürüm karşılaştırması etiketten sayısal bölümleri okur. `CD-625` gibi sürüm numarası taşımayan etiketler yanlış `eski` sonucu verebilir.
- Aday arşiv URL'si, eski etiketin yenisiyle değiştirilmesiyle üretilen bir tahmindir. Doğrulanmamıştır.
- GitHub API sınırına takılınırsa kalan paketler `hata: rate limit` olarak işaretlenir ve rapor yine yazılır.
