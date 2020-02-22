.PHONY: inspect

test.blk:
	cargo run -- create $@ ../src/*

inspect: test/.blk
	cargo run -- inspect $<