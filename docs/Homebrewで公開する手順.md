# Homebrewで公開する手順

## 1. GitHub Releaseを作る

例: `v0.1.0` タグを作成して push する

```bash
git tag v0.1.0
git push origin v0.1.0
```

## 2. Homebrew tapリポジトリを用意する

- リポジトリ名: `homebrew-clip-show`
- 例: `https://github.com/somei-san/homebrew-clip-show`

## 3. Formulaを生成する

このリポジトリで以下を実行:

```bash
./scripts/homebrew/generate_formula.sh somei-san {バージョン（exp: 0.0.1）} ./Formula/clip-show.rb
```

生成された `Formula/clip-show.rb` を tap リポジトリの `Formula/clip-show.rb` としてコミットして push してください。

テンプレートは `packaging/homebrew/clip-show.rb.template` にあります。

## 4. ユーザーのインストール手順

```bash
brew tap somei-san/clip-show
brew install clip-show
```

## PC起動時に常駐起動する

Homebrew service として登録すると、ログイン時に自動起動します。

```bash
brew services start clip-show
```

停止:

```bash
brew services stop clip-show
```

補足:
- `clip-show` はGUI（AppKit）アプリのため、ユーザーログインセッションで動かしてください。
