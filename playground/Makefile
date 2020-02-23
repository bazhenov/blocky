.PHONY: inspect

TEST_BLOCK=test.blk

clean: 
	rm -f ${TEST_BLOCK}

test.blk: clean
	cargo run -- create $@ ../src/*

inspect: ${TEST_BLOCK}
	cargo run -- inspect -v $<