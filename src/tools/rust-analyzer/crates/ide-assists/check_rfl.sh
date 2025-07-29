#!/bin/bash
echo -e "rfl.in\nrfl.out" | cargo test rfl_add_missing_impl -- --nocapture

if ! grep 'todo!' rfl.out; then
	echo "failed"
	exit 1
fi

echo ok
exit 0
