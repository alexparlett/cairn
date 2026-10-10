# Create Branch's Discard and branch names against real git (2026-10-10)

Evidence for phase 14's stopping rule and the user's decision of 2026-10-10 on it. Every case was run
in a throwaway repository (`GIT_CONFIG_NOSYSTEM=1`, a scratch `HOME`) on git 2.56.0 (the host),
2.30.9 and 2.32.7 (the floors, `~/.cache/cairn/git-floor`); the three versions answered alike in
every case. The scripts are reproduced at the end.

## The forced checkout: `git checkout -q --no-track -f -b <name> <oid> --`

| Case | Exit | Afterwards |
| --- | --- | --- |
| A staged edit, an unstaged edit, a staged new file, an untracked file at a path the target holds, an untracked folder beside a target folder, an untracked file elsewhere | 0 | on the new branch; the staged and unstaged edits and the staged new file gone; the untracked file at the target's path overwritten with the target's content; the untracked files the target holds nothing at kept (`?? sub/other`, `?? u.txt`) |
| A merge in progress with a conflicted path (`UU a.txt`, `MERGE_HEAD` and `MERGE_MSG` present) | 0 | on the new branch; the conflict gone, the tree clean; `MERGE_HEAD` and `MERGE_MSG` removed — the merge silently abandoned |
| A conflicted path with no operation in progress (a `git stash apply` conflict, `UU a.txt`) | 0 | on the new branch; the conflict gone, the tree clean |
| A submodule whose checked-out commit moved (` M m`) | 0 | still ` M m`; the submodule still on the moved commit |
| A submodule with an edit inside it (` M m`) | 0 | still ` M m`; the edit inside the submodule kept |
| A staged change of a submodule's commit (`M  m`) | 0 | ` M m`: the index entry put back, the submodule's checkout left on the moved commit, so the change is listed again as unstaged |

What this says, as the user decided on it (2026-10-10, "Agree take them"): git refuses none of these.
Over a submodule it leaves the submodule's change in place, as Fork's command (the same command,
observed in `fork-observed-2026-10-10.md`, "Create Branch") would, and Local Changes then still lists
it; over a conflicted path with no operation in progress it discards the conflict, as Fork's would;
during an operation it abandons the operation without a word, which is why Cairn refuses an
operation in progress before git runs.

## Branch names: git's verbs against `check-ref-format`

One fresh repository per name, holding `main`, `prev` (so `@{-1}` is `prev` and `@{-2}` is
`main`), `taken`, `folder/inner` and `leaf`.

| Name | `git branch -- <name> <oid>` | `git checkout -b <name> <oid> --` | `check-ref-format --branch` | `check-ref-format refs/heads/<name>` |
| --- | --- | --- | --- | --- |
| `@{-1}` | refused: a branch named 'prev' already exists | refused, the same | prints `prev` | refused |
| `@{-2}` | refused: a branch named 'main' already exists | refused, the same | prints `main` | refused |
| `x@{y` | refused: not a valid branch name | refused, the same | refused | refused |
| `a..b` | refused: not a valid branch name | refused, the same | refused | refused |
| `taken` | refused: already exists | refused, the same | accepted | accepted |
| `folder` (`folder/inner` exists) | refused: cannot lock ref … 'refs/heads/folder/inner' exists | refused, the same | accepted | accepted |
| `leaf/child` (`leaf` exists) | refused: cannot lock ref … 'refs/heads/leaf' exists | refused, the same | accepted | accepted |
| `-x` | refused: not a valid branch name | refused, the same | refused | **accepted** |
| `HEAD` | refused: not a valid branch name | refused, the same | refused | **accepted** |
| `@` | created | created | accepted | accepted |

git's verbs resolve `@{-N}` exactly as `check-ref-format --branch` does (both read the name through
git's branch-name interpretation), so `--branch` is not the wrong oracle the review's M3 proposed;
`check-ref-format refs/heads/<name>` is, accepting `-x` and `HEAD`, which both verbs refuse. A
taken name and a folder clash are not the format's to answer, and Cairn looks them up itself. Cairn
refuses a name holding `@{` before git is asked (the user's decision F, 2026-10-09), so a name that
git would read as another branch's never reaches git.

## The scripts

### `probe_checkout.sh <git>`

```sh
#!/bin/bash
# usage: probe_checkout.sh <git>
G="$1"
export GIT_CONFIG_NOSYSTEM=1 HOME=$PWD/home GIT_AUTHOR_NAME=a GIT_AUTHOR_EMAIL=a@a GIT_COMMITTER_NAME=a GIT_COMMITTER_EMAIL=a@a
mkdir -p home
W=$(mktemp -d -p $PWD)
cd $W
$G init -q -b main r && cd r
echo base > a.txt; echo keep > b.txt; mkdir dir; echo x > dir/f; $G add . ; $G commit -qm base
BASE=$($G rev-parse HEAD)
echo target > t.txt; mkdir sub; echo s > sub/file; $G add .; $G commit -qm target
TARGET=$($G rev-parse HEAD)
$G checkout -q $BASE 2>/dev/null; $G checkout -q -B main $BASE
echo "== case 1: staged, unstaged, untracked in the way, untracked elsewhere"
echo staged >> a.txt; $G add a.txt; echo unstaged >> b.txt
echo newstaged > n.txt; $G add n.txt
echo mine > t.txt   # untracked where target has a file
mkdir sub; echo mine > sub/other   # untracked folder where target has a dir (kept? only file-held overwritten)
echo elsewhere > u.txt
$G checkout -q --no-track -f -b topic $TARGET -- ; echo "exit $?"
echo "HEAD: $($G symbolic-ref HEAD)"; $G status --porcelain=v1 -uall; echo "t.txt: $(cat t.txt)"; ls sub
cd ..; rm -rf r
$G init -q -b main r && cd r
echo base > a.txt; $G add .; $G commit -qm base; BASE=$($G rev-parse HEAD)
$G checkout -q -b other; echo other > a.txt; $G commit -qam other
$G checkout -q main; echo mainside > a.txt; $G commit -qam mainside
echo "== case 2: conflicted index from a merge"
$G merge -q other >/dev/null 2>&1; $G status --porcelain=v1
$G checkout -q --no-track -f -b topic2 $BASE -- ; echo "exit $?"
echo "HEAD: $($G symbolic-ref HEAD)"; $G status --porcelain=v1; ls .git | grep -i merge; cat a.txt
$G checkout -q -f main; $G branch -D -q topic2
echo "== case 3: conflicted index with no operation (stash apply conflict)"
echo stashme > a.txt; $G stash -q; echo conflicting > a.txt; $G commit -qam c3; $G stash apply -q >/dev/null 2>&1; $G status --porcelain=v1; ls .git | grep -i -E "merge|cherry|revert|rebase"
$G checkout -q --no-track -f -b topic3 $BASE -- ; echo "exit $?"
echo "HEAD: $($G symbolic-ref HEAD)"; $G status --porcelain=v1; cat a.txt
cd ..; rm -rf r
echo "== case 4: submodule changed"
$G init -q -b main subrepo; (cd subrepo; echo 1 > s; $G add s; $G commit -qm s1; echo 2 > s; $G commit -qam s2)
$G init -q -b main r && cd r
echo a > a.txt; $G add a.txt; $G commit -qm a
$G -c protocol.file.allow=always submodule add -q ../subrepo m >/dev/null 2>&1; $G commit -qm sub
BASE=$($G rev-parse HEAD)
(cd m; $G checkout -q HEAD~1)   # submodule HEAD moved
echo "-- 4a submodule HEAD moved (unstaged gitlink)"; $G status --porcelain=v1
$G checkout -q --no-track -f -b t4a $BASE -- ; echo "exit $?"; $G status --porcelain=v1; echo "sub HEAD: $(cd m; $G log -1 --format=%s)"
(cd m; $G checkout -q main; echo dirty > s)
echo "-- 4b submodule dirty inside"; $G status --porcelain=v1
$G checkout -q --no-track -f -b t4b $BASE -- ; echo "exit $?"; $G status --porcelain=v1; echo "sub s: $(cat m/s)"
(cd m; $G checkout -q -f main; $G checkout -q HEAD~1); $G add m
echo "-- 4c staged gitlink change"; $G status --porcelain=v1
$G checkout -q --no-track -f -b t4c $BASE -- ; echo "exit $?"; $G status --porcelain=v1; echo "sub HEAD: $(cd m; $G log -1 --format=%s)"
cd /; rm -rf $W
```

### `probe_merge.sh <git>`

```sh
#!/bin/bash
G="$1"
export GIT_CONFIG_NOSYSTEM=1 HOME=$PWD/home GIT_AUTHOR_NAME=a GIT_AUTHOR_EMAIL=a@a GIT_COMMITTER_NAME=a GIT_COMMITTER_EMAIL=a@a
W=$(mktemp -d -p $PWD); cd $W
$G init -q -b main r && cd r
echo base > a.txt; $G add .; $G commit -qm base; BASE=$($G rev-parse HEAD)
$G checkout -q -b other; echo other > a.txt; $G commit -qam other
$G checkout -q main; echo mainside > a.txt; $G commit -qam mainside
$G merge -q other >/dev/null 2>&1
echo "before: $(ls .git | grep -E 'MERGE_HEAD|MERGE_MSG' | tr '\n' ' ') status: $($G status --porcelain=v1 | tr '\n' ' ')"
$G checkout -q --no-track -f -b t $BASE -- ; echo "exit $?"
echo "after: [$(ls .git | grep -E 'MERGE_HEAD|MERGE_MSG|AUTO_MERGE' | tr '\n' ' ')] status: [$($G status --porcelain=v1 | tr '\n' ' ')] HEAD $($G symbolic-ref --short HEAD)"
cd /; rm -rf $W
```

### `probe_names.sh <git>`

```sh
#!/bin/bash
G="$1"
export GIT_CONFIG_NOSYSTEM=1 HOME=$PWD/home GIT_AUTHOR_NAME=a GIT_AUTHOR_EMAIL=a@a GIT_COMMITTER_NAME=a GIT_COMMITTER_EMAIL=a@a
W=$(mktemp -d -p $PWD); cd $W
fresh() { rm -rf r; $G init -q -b main r; cd r; echo a > a; $G add a; $G commit -qm a
$G branch -q prev; $G checkout -q prev; $G checkout -q main
$G branch -q taken; $G branch -q folder/inner; $G branch -q leaf; H=$($G rev-parse HEAD); }
for n in '@{-1}' '@{-2}' 'x@{y' 'a..b' 'taken' 'folder' 'leaf/child' '-x' '@' 'HEAD'; do
  fresh; b=$($G branch -- "$n" $H 2>&1 >/dev/null; echo "rc=$?"); cd ..
  fresh; c=$($G checkout -q -b "$n" $H -- 2>&1 >/dev/null; echo "rc=$?; now $($G symbolic-ref --short HEAD) [$($G branch --list|tr -d ' *'|tr '\n' ,)]"); cd ..
  fresh; r1=$($G check-ref-format --branch "$n" 2>&1; echo "rc=$?"); r2=$($G check-ref-format "refs/heads/$n" 2>&1; echo "rc=$?"); cd ..
  echo "[$n] branch: $(echo $b|tr '\n' ' '|cut -c1-90) | checkout -b: $(echo $c|tr '\n' ' '|cut -c1-120) | --branch: $(echo $r1|tr '\n' ' ') | refs/heads: $r2"
done
cd /; rm -rf $W
```
