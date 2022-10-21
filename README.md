# Build docker image

docker build . -t mailcrab

# Run

docker run -it --rm -p 8080:8080 -p 2525:2525 mailcrab
