# Homebrewで公開する手順

## 1. GitHub Releaseを作る

例: `v0.1.0` タグを作成して push する

```bash
git tag v0.1.0
git push origin v0.1.0
```

## 2. Homebrew tapリポジトリ

<https://github.com/somei-san/homebrew-tools>

## 3. Formulaを生成する

このリポジトリで以下を実行:

```bash
./scripts/homebrew/generate_formula.sh somei-san {バージョン(exp: 0.1.0)} ./Formula/cliip-show.rb
```

生成された `Formula/cliip-show.rb` を [tap リポジトリ](##-2.-Homebrew-tapリポジトリ)の `Formula/cliip-show.rb` としてコミットして push してください。

テンプレートは `packaging/homebrew/cliip-show.rb.template` にあります。

## 4. ユーザーのインストール手順

[TapリポジトリのREADME参照](https://github.com/somei-san/homebrew-tools/blob/main/README.md)

## 補足

`cliip-show` はGUI（AppKit）アプリのため、ユーザーログインセッションで動かしてください。
