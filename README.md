# clip-show

## 背景

コピーしたと思ったのにできてなかった！ペーストしたら意図したコピー内容と違った！！
そんなことありませんか？
ありますよねぇ〜

てなわけで、
クリップボードにコピーしたときにコピー内容が表示されるアプリ
のソースコードです

## 概要

コピーされたプレーンテキストを画面中央に1秒だけHUD表示する、macOS常駐アプリです。

## 開発起動

```bash
cargo run
```

## .app化

```bash
cargo install cargo-bundle
cargo bundle --release
open target/release/bundle/osx/clip-show.app
```

## Homebrewリポジトリ

https://github.com/somei-san/homebrew-clip-show

[README](https://github.com/somei-san/homebrew-clip-show/README.md)

## ドキュメント

- Homebrew公開と常駐起動の手順: `docs/homebrew.md`
