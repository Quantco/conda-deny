#!/bin/bash

# The following can be used to rerender the vhs demo tapes in .github/assets/demo

# Make sure you are in an environment with vhs installed

export PS1="> "
vhs .github/assets/demo/demo-light.tape
vhs .github/assets/demo/demo-dark.tape