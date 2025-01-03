Output .github/assets/demo/demo-dark.mp4
Output .github/assets/demo/demo-dark.gif

Set Theme "3024 Night"
Set Shell "bash"
Set FontSize 32
Set Width 1920
Set Height 750

Hide
Type "cd tests/test_default_use_case" Enter
Type "clear" Enter

Show
Type@150ms "ls" Sleep 1s Enter
Sleep 3s

Type@150ms "conda-deny --help" Sleep 1s Enter
Sleep 5s

Type "clear" Sleep 1s Enter
Sleep 3s

Type "cat license_whitelist.toml" Sleep 1s Enter
Sleep 3s

Type@150ms "conda-deny check" Sleep 1s Enter
Sleep 6s