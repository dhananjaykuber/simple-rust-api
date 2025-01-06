use postgres::Error as PostgresError;
use postgres::{Client, NoTls};
use std::env;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

/**
 * - `serde` is a serialization/deserialization library for Rust
 */
#[macro_use]
extern crate serde_derive;

// Model: User struct
#[derive(Serialize, Deserialize)]
struct User {
    id: Option<i32>,
    name: String,
    email: String,
}

// DATABASE URL
const DB_URL: &str = env!("DATABASE_URL");

// Response constants
const OK_RESPONSE: &str = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n";
const NOT_FOUND: &str = "HTTP/1.1 404 NOT FOUND\r\n\r\n";
const INTERNAL_ERROR: &str = "HTTP/1.1 500 INTERNAL ERROR\r\n\r\n";

// Main function
fn main() {
    // Set up database
    /**
     * - Calls the set_database() function and checks if it returns an error
     * - `if let` is a pattern matching construct that only executes the code block if the pattern matches
     * - Err(_) means "if there's an error
     * - If there's an error, print "Error setting database" and return
     * - If there's no error, continue
     */
    if let Err(_) = set_database() {
        println!("Error setting database");
        return;
    }

    // Create a listener on port 3000
    /**
     * - `TcpListener::bind()` creates a server that can accept connections
     * - 0.0.0.0 means "listen on all available network interfaces"
     * - unwrap() gets the result value or crashes if there's an error
     */
    let listener = TcpListener::bind(format!("0.0.0.0:3000")).unwrap();
    println!("Server listening on port 3000");

    // Listen for incoming connections
    /**
     * - `listener.incoming()` returns an iterator over incoming connections
     * - `for` loops over each incoming connection
     * - `match` is a pattern matching construct
     * - Each connection is a Result type that might be either
     *  - Ok(stream) if the connection is successful
     *  - Err(e) if there's an error
     * - If the connection is successful, call the `handle_client()` function to handle the request
     */
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                handle_client(stream);
            }
            Err(e) => {
                println!("Unable to connect: {}", e);
            }
        }
    }
}

// Handle client request
/**
 * - Takes a mutable TCP stream as parameter (marked mut because we need to read from and write to it)
 */
fn handle_client(mut stream: TcpStream) {
    /**
     * - Creates a buffer array of 1024 zeros to temporarily store incoming data
     * - Creates an empty string to store the request
     */
    let mut buffer = [0; 1024];
    let mut request = String::new();

    // Reads data from the stream into our buffer
    match stream.read(&mut buffer) {
        /**
         * If read is successful:
         * - `size` is how many bytes were read
         * - `&buffer[..size]` takes a slice of the buffer up to the number of bytes read
         * - `String::from_utf8_lossy()` converts the slice to a string
         * - `.as_ref()` gets a reference to the string
         * - `push_str()` appends the string to the request
         */
        Ok(size) => {
            request.push_str(String::from_utf8_lossy(&buffer[..size]).as_ref());

            /**
             * - Uses pattern matching to check the start of the request string
             * - Calls the appropriate function based on the request
             * - The pattern r if r.starts_with(...) binds the request to r and checks if it starts with the given path
             */
            let (status_line, content) = match &*request {
                r if r.starts_with("POST /users") => handle_post_request(r),
                r if r.starts_with("GET /users/") => handle_get_request(r),
                r if r.starts_with("GET /users") => handle_get_all_request(r),
                r if r.starts_with("PUT /users/") => handle_put_request(r),
                r if r.starts_with("DELETE /users/") => handle_delete_request(r),
                _ => (NOT_FOUND.to_string(), "404 not found".to_string()),
            };

            stream
                .write_all(format!("{}{}", status_line, content).as_bytes())
                .unwrap();
        }
        Err(e) => eprintln!("Unable to read stream: {}", e),
    }
}

// Handle post request
/**
 * - Takes a string as parameter and returns a tuple of two strings (status_line, content)
 */
fn handle_post_request(request: &str) -> (String, String) {
    /**
     * This match block does the following:
     * - Calls the get_user_request_body() function to get the user data from the request body
     * - Calls the Client::connect() function to connect to the database
     * - Both operations return Result types, which is why we use match
     */
    match (
        get_user_request_body(&request),
        Client::connect(DB_URL, NoTls),
    ) {
        /**
         * - Ok(user) means we successfully parsed the User from request
         * - Ok(mut client) means we successfully connected to the database
         */
        (Ok(user), Ok(mut client)) => {
            client
                .execute(
                    "INSERT INTO users (name, email) VALUES ($1, $2)",
                    &[&user.name, &user.email],
                )
                .unwrap();

            (OK_RESPONSE.to_string(), "User created".to_string())
        }
        _ => (INTERNAL_ERROR.to_string(), "Internal error".to_string()),
    }
}

//handle get request
fn handle_get_request(request: &str) -> (String, String) {
    match (
        get_id(&request).parse::<i32>(),
        Client::connect(DB_URL, NoTls),
    ) {
        (Ok(id), Ok(mut client)) => {
            match client.query_one("SELECT * FROM users WHERE id = $1", &[&id]) {
                Ok(row) => {
                    let user = User {
                        id: row.get(0),
                        name: row.get(1),
                        email: row.get(2),
                    };

                    (
                        OK_RESPONSE.to_string(),
                        serde_json::to_string(&user).unwrap(),
                    )
                }
                _ => (NOT_FOUND.to_string(), "User not found".to_string()),
            }
        }

        _ => (INTERNAL_ERROR.to_string(), "Internal error".to_string()),
    }
}

//handle get all request
fn handle_get_all_request(_request: &str) -> (String, String) {
    match Client::connect(DB_URL, NoTls) {
        Ok(mut client) => {
            let mut users = Vec::new();

            for row in client
                .query("SELECT id, name, email FROM users", &[])
                .unwrap()
            {
                users.push(User {
                    id: row.get(0),
                    name: row.get(1),
                    email: row.get(2),
                });
            }

            (
                OK_RESPONSE.to_string(),
                serde_json::to_string(&users).unwrap(),
            )
        }
        _ => (INTERNAL_ERROR.to_string(), "Internal error".to_string()),
    }
}

// Handle put request
fn handle_put_request(request: &str) -> (String, String) {
    match (
        get_id(&request).parse::<i32>(),
        get_user_request_body(&request),
        Client::connect(DB_URL, NoTls),
    ) {
        (Ok(id), Ok(user), Ok(mut client)) => {
            client
                .execute(
                    "UPDATE users SET name = $1, email = $2 WHERE id = $3",
                    &[&user.name, &user.email, &id],
                )
                .unwrap();

            (OK_RESPONSE.to_string(), "User updated".to_string())
        }
        _ => (INTERNAL_ERROR.to_string(), "Internal error".to_string()),
    }
}

// Handle delete request
fn handle_delete_request(request: &str) -> (String, String) {
    match (
        get_id(&request).parse::<i32>(),
        Client::connect(DB_URL, NoTls),
    ) {
        (Ok(id), Ok(mut client)) => {
            let rows_affected = client
                .execute("DELETE FROM users WHERE id = $1", &[&id])
                .unwrap();

            //if rows affected is 0, user not found
            if rows_affected == 0 {
                return (NOT_FOUND.to_string(), "User not found".to_string());
            }

            (OK_RESPONSE.to_string(), "User deleted".to_string())
        }
        _ => (INTERNAL_ERROR.to_string(), "Internal error".to_string()),
    }
}

// Set up database
fn set_database() -> Result<(), PostgresError> {
    let mut client = Client::connect(DB_URL, NoTls)?;
    client.batch_execute(
        "
        CREATE TABLE IF NOT EXISTS users (
            id SERIAL PRIMARY KEY,
            name VARCHAR NOT NULL,
            email VARCHAR NOT NULL
        )
    ",
    )?;
    Ok(())
}

// Get id from request
/**
 * - Splits the request string by "/" and gets the third element (For "GET /users/123 HTTP/1.1" becomes: ["GET ", "users", "123 HTTP/1.1"]) and gets "123 HTTP/1.1"
 * - Splits the string by whitespace and gets the first element (For "123 HTTP/1.1" becomes: ["123", "HTTP/1.1"]) and gets "123"
 * - Returns the id as a string
 * - .unwrap_or_default() returns an empty string if the value is None
 */
fn get_id(request: &str) -> &str {
    request
        .split("/")
        .nth(2)
        .unwrap_or_default()
        .split_whitespace()
        .next()
        .unwrap_or_default()
}

// Get user request body
/**
 * - Splits the request string by "\r\n\r\n" and gets the last element (For "POST /users HTTP/1.1\r\nContent-Type: application/json\r\n\r\n{\"name\":\"John\",\"email\":\"
 * - Parses the JSON string into a User struct
 * - Returns the User struct
 */
fn get_user_request_body(request: &str) -> Result<User, serde_json::Error> {
    serde_json::from_str(request.split("\r\n\r\n").last().unwrap_or_default())
}
