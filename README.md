[![progress-banner](https://backend.codecrafters.io/progress/git/91ce31ab-0cf4-4ba1-9024-9c8dc9f3ea80)](https://app.codecrafters.io/users/codecrafters-bot?r=2qF)

This is a starting point for Rust solutions to the
["Build Your Own Git" Challenge](https://codecrafters.io/challenges/git).

In this challenge, you'll build a small Git implementation that's capable of
initializing a repository, creating commits and cloning a public repository.
Along the way we'll learn about the `.git` directory, Git objects (blobs,
commits, trees etc.), Git's transfer protocols and more.

**Note**: If you're viewing this repo on GitHub, head over to
[codecrafters.io](https://codecrafters.io) to try the challenge.

# Passing the first stage

The entry point for your Git implementation is in `src/main.rs`. Study and
uncomment the relevant code, and push your changes to pass the first stage:

```sh
git commit -am "pass 1st stage" # any msg
git push origin master
```

That's all!

# Stage 2 & beyond

Note: This section is for stages 2 and beyond.

1. Ensure you have `cargo (1.94)` installed locally
1. Run `./your_program.sh` to run your Git implementation, which is implemented
   in `src/main.rs`. This command compiles your Rust project, so it might be
   slow the first time you run it. Subsequent runs will be fast.
1. Commit your changes and run `git push origin master` to submit your solution
   to CodeCrafters. Test output will be streamed to your terminal.

# Testing locally

The `your_program.sh` script is expected to operate on the `.git` folder inside
the current working directory. If you're running this inside the root of this
repository, you might end up accidentally damaging your repository's `.git`
folder.

We suggest executing `your_program.sh` in a different folder when testing
locally. For example:

```sh
mkdir -p /tmp/testing && cd /tmp/testing
/path/to/your/repo/your_program.sh init
```

To make this easier to type out, you could add a
[shell alias](https://shapeshed.com/unix-alias/):

```sh
alias mygit=/path/to/your/repo/your_program.sh

mkdir -p /tmp/testing && cd /tmp/testing
mygit init
```

## Commands
### Init
mkdir some_dir && cd some_dir \
cargo run -- init

### cat-file
mkdir some_dir && cd some_dir \
/path/to/your_program.sh init \
echo "hello world" > test.txt # The tester will use a random string, not "hello world" \
git hash-object -w test.txt \
3b18e512dba79e4c8300dd08aeb37f8e728b8dad \
/path/to/your_program.sh cat-file -p 3b18e512dba79e4c8300dd08aeb37f8e728b8dad \
hello world \

### hash-object
mkdir test_dir && cd test_dir \
/path/to/your_program.sh init \
echo "hello world" > test.txt \
./your_program.sh hash-object -w test.txt \
3b18e512dba79e4c8300dd08aeb37f8e728b8dad \

### ls-tree
mkdir test_dir && cd test_dir \
/path/to/your_program.sh init \
It'll then write a tree object to the .git/objects directory. \
/path/to/your_program.sh ls-tree --name-only <tree_sha> \

### write-tree
echo "hello world" > test.txt \
git add test.txt \
git write-tree \
4b825dc642cb6eb9a060e54bf8d69288fbee4904 \

### commit-tree
mkdir test_dir && cd test_dir \
git init \
echo "hello world" > test.txt \
git add test.txt \
git write-tree \
4b825dc642cb6eb9a060e54bf8d69288fbee4904 \
git commit-tree 4b825dc642cb6eb9a060e54bf8d69288fbee4904 -m "Initial commit" \
3b18e512dba79e4c8300dd08aeb37f8e728b8dad \
echo "hello world 2" > test.txt \
git add test.txt \
git write-tree \
5b825dc642cb6eb9a060e54bf8d69288fbee4904 \
git commit-tree 5b825dc642cb6eb9a060e54bf8d69288fbee4904 -p 3b18e512dba79e4c8300dd08aeb37f8e728b8dad -m "Second commit" \
6c18e512dba79e4c8300dd08aeb37f8e728b8dad \

### clone
/path/to/your_program.sh clone https://github.com/blah/blah <some_dir> \
