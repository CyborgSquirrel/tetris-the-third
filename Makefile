.PHONY: RUST_SRC run

run: target/release/tetris
	./target/release/tetris

target/release/tetris: RUST_SRC
	cargo build --release
	cp ./lib/libfreetype-6.dll ./target/release/
	cp ./lib/libpng16-16.dll   ./target/release/
	cp ./lib/SDL2.dll          ./target/release/
	cp ./lib/SDL2_image.dll    ./target/release/
	cp ./lib/SDL2_ttf.dll      ./target/release/
	cp ./lib/zlib1.dll         ./target/release/

