#!/bin/bash

# The following can be used to rerender the vhs demo tapes in .github/assets/demo

# Make sure you are in an environment with vhs installed

export PS1="> "
vhs demo-light.tape
vhs demo-dark.tape