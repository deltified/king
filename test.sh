#!/bin/bash

OS_TYPE="$(uname -s)"

if [ "$OS_TYPE" = "Linux" ]; then
    
    # Linux sometimes requires the path to be set manually
    export LLVM_SYS_220_PREFIX=/usr/lib/llvm-22
    export PATH=/usr/lib/llvm-22/bin:$PATH
    export LIBRARY_PATH=/usr/lib/llvm-22/lib:$LIBRARY_PATH

    cargo test -- --nocapture

elif [[ "$OS_TYPE" == "Darwin" || "$OS_TYPE" == "MINGW"* || "$OS_TYPE" == "CYGWIN"* || "$OS_TYPE" == "MSYS"* ]]; then
    cargo test -- --nocapture
else
    echo "Unknown and potentially unsupported OS: $OS_TYPE. Defauling to standard cargo test but there is no guarantees it will work."
    cargo test -- --nocapture
fi