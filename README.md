# what-the-port

Quickly lookup what a port is used for.

[![Demo](https://asciinema.org/a/eZpsErATwmm82hcHxrKxROz5C.svg)](https://asciinema.org/a/eZpsErATwmm82hcHxrKxROz5C)

## Install

This tool is very new, so it hasn't been packaged in many places.

If you would like to help package it, please do so and submit an issue/PR so we can list it here.

### AUR

`paru -S what-the-port`

### Build locally

`cargo install what-the-port`

## How it works

In essence, this program is a scraper + parser that takes the information in the excellent Wikipedia page
[List of TCP and UDP port numbers](https://en.wikipedia.org/wiki/List_of_TCP_and_UDP_port_numbers)
and presents it after formatting the data. There is really nothing special about it.

The main sell, as is the case for many CLI tools, is that of convenience.
Instead of having to open up a browser, navigate to the page and scroll or search,
you can do it from the comfort of your terminal with one command.
There is also the additional benefit that you can now do this lookup offline thanks to caching.

Of course, the intrinsic issue with any scraper is that it isn't particularly resilient.
Wikipedia is intended for human consumption first and foremost, not for a program.
I fully expect this tool to have issues regularly, when the Wikipedia page gets updated
in an unhandled way. Therefore most aspects of this tool are written to be fail-safe,
meaning it will try its best to produce usable output. If you encounter errors or warnings,
please report them as issues.
