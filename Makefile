run:
	@make -C kernel run

clean:
	@make -C kernel clean

fmt:
	@cd os && cargo fmt