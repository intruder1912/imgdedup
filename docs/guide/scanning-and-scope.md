# Scanning and scope

[← Guide index](README.md)

`imgdedup` takes one or more paths to scan and decides which files are *candidate*
images before it ever looks at content. This page covers the flags that control
that scope.

```text
imgdedup [OPTIONS] <PATH>...
```

`<PATH>...` is one or more files or directories. Directories are scanned
recursively by default.

## Recursion: `--no-recurse`

By default `imgdedup` descends into subdirectories. Pass `--no-recurse` to scan
only the top level of each path:

```sh
imgdedup --no-recurse ./inbox
```

## Hidden files: `--hidden`

Files and directories whose names start with `.` are skipped by default. Include
them with `--hidden`:

```sh
imgdedup --hidden ./photos
```

A hidden path that you pass *explicitly* as a scan root is always included — the
hidden filter only applies to entries discovered during the walk.

## Symbolic links: `--follow-symlinks`

Symbolic links are not followed by default. Pass `--follow-symlinks` to traverse
them:

```sh
imgdedup --follow-symlinks ./library
```

## Extension allow-list: `--ext`

Only files whose extension is on the image allow-list are considered. The default
list is:

```text
jpg  jpeg  png  gif  webp  bmp  tif  tiff
```

Matching is case-insensitive. Replace the list entirely with `--ext`, giving a
comma- or space-separated set (leading dots are tolerated):

```sh
imgdedup --ext "png,webp" ./screenshots
```

> The extension is only a first filter. After it, every candidate's **content** is
> confirmed to be a real image by its magic bytes, so a text file renamed
> `photo.jpg` is rejected and never acted on. A file whose content cannot be
> confirmed as a supported image is skipped (the safe direction).

## Next

- [Choosing what to keep](choosing-what-to-keep.md)
- [Removing duplicates safely](removing-duplicates-safely.md)
