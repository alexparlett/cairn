# The ui-review fixture repository

One repository, built the same way inside the Fork VM (Git Bash) and on the
host for Cairn, so the two windows are compared over the same history, refs and
working-tree state. Run from an empty parent directory (`C:\uirev` in the VM).

```sh
git init -q -b main --bare remote.git
git init -q -b main t && cd t
git config user.name "Ada Lovelace"; git config user.email ada@example.com
mkdir src
printf 'fn main() {\n    println!("hello");\n}\n' > src/main.rs
printf '# Demo\n\nA demo repository.\n' > README.md
git add -A; git commit -qm "Initial commit"
for i in 1 2 3 4 5 6; do echo "line $i" >> notes.txt; git add -A; git commit -qm "Add note $i"; done
git tag v0.1
git checkout -qb feature/login
echo "pub fn login() {}" > src/login.rs; git add -A; git commit -qm "Add login module"
echo "// validate" >> src/login.rs; git commit -qam "Validate login input"
git checkout -q main
echo "Fixed." >> README.md; git commit -qam "Fix README typo"
git merge -q --no-ff feature/login -m "Merge branch 'feature/login'"
git tag -a v0.2 -m "Release 0.2"
git checkout -qb bugfix/crash
echo "// guard" >> src/main.rs; git commit -qam "Fix crash on empty input"
git checkout -q main
git remote add origin ../remote.git
git push -qu origin main feature/login
echo "line 7" >> notes.txt; git commit -qam "Local only commit"
echo "// wip" >> src/main.rs; git stash push -qm "WIP on parser"
echo "Edited." >> README.md
echo "// staged" >> src/login.rs; git add src/login.rs
echo "untracked" > new.txt
```

End state: `main` checked out, one commit ahead of `origin/main`; branches
`feature/login` (tracking `origin/feature/login`) and `bugfix/crash`; tags
`v0.1` (lightweight) and `v0.2` (annotated); one stash; a merge in the graph;
Local Changes holding one unstaged modification (`README.md`), one staged
modification (`src/login.rs`) and one untracked file (`new.txt`).
