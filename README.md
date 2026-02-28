# cliip-show

コピーしたと思ったのにできてなかった 😡

ペーストしたら意図したコピー内容と違った 😡 😡

そんなことありませんか？

ありますよねぇ〜 🤓

てなわけで、

コピーされたプレーンテキストを画面中央に表示する、macOS向けの常駐アプリ`cliip-show`です 🐟

## 概要

- クリップボードの更新を監視し、コピー直後にHUD表示します
- HUDは数秒で自動的に消えます
- アプリはバックグラウンドで常駐して動作します

## 動作環境

- macOS（AppKitを使用）
- Homebrew（通常利用時のインストール手段）

## HUDイメージ

![cliip-show HUDの表示イメージ](docs/assets/cliip-show-hud.png)

## インストール手順 （Homebrew経由）

```bash
brew tap somei-san/tools
brew install somei-san/tools/cliip-show
brew services start cliip-show
```

## ドキュメント

- [開発者向け手順](docs/development.md)

## Homebrew tap リポジトリ

<https://github.com/somei-san/homebrew-tools>
