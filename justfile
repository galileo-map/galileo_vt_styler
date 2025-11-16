set dotenv-load := true
export VT_API_KEY := env('VT_API_KEY', "")

run:
  cargo run