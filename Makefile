.PHONY: RUST_SRC run

run: target/release/tetris
	./target/release/tetris

ifeq ($(OS),Windows_NT)

target/release/tetris: RUST_SRC
	cargo build --release
	copy .\\lib\\libfreetype-6.dll .\\target\\release\\
	copy .\\lib\\libpng16-16.dll   .\\target\\release\\
	copy .\\lib\\SDL2.dll          .\\target\\release\\
	copy .\\lib\\SDL2_image.dll    .\\target\\release\\
	copy .\\lib\\SDL2_ttf.dll      .\\target\\release\\
	copy .\\lib\\zlib1.dll         .\\target\\release\\

else

target/release/tetris: RUST_SRC
	cargo build --release
	cp ./lib/libfreetype-6.dll ./target/release/
	cp ./lib/libpng16-16.dll   ./target/release/
	cp ./lib/SDL2.dll          ./target/release/
	cp ./lib/SDL2_image.dll    ./target/release/
	cp ./lib/SDL2_ttf.dll      ./target/release/
	cp ./lib/zlib1.dll         ./target/release/

endif