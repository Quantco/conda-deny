Output .github/assets/demo/demo-light.mp4
Output .github/assets/demo/demo-light.gif

Set Theme "3024 Day"
Set Shell "bash"
Set FontSize 32
Set Width 2020
Set Height 750

Hide
Type "cd tests/test_default_use_case" Enter
Type "alias conda-deny='$HOME/Projects/conda-deny/target/debug/conda-deny'" Enter
Type "export PS1='$ '" Enter
Type "clear" Enter

Show
Type@150ms "ls" Sleep 1s Enter
Sleep 3s

Type@150ms "conda-deny --help" Sleep 1s Enter
Sleep 5s

Type "clear" Sleep 1s Enter
Sleep 3s

Type "bat license_allowlist.toml" Sleep 1s Enter
Sleep 3s

Type@150ms "conda-deny check" Sleep 1s Enter
Sleep 5s

Type@150ms "conda-deny bundle --prefix ../test_conda_prefixes/test-env" Sleep 1s Enter
Sleep 7s

Type@150ms "tree .conda-deny" Sleep 1s Enter
Sleep 5s
