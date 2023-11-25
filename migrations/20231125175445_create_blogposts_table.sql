-- migrations/{timestamp}_create_blogposts_table.sql

-- Create an ENUM type for blog post status
CREATE TYPE blogpost_status AS ENUM ('draft', 'published', 'archived');

-- Create Blogposts table
CREATE TABLE blogposts(
    id uuid NOT NULL,
    PRIMARY KEY (id),
    created_at timestamptz NOT NULL,
    updated_at timestamptz,
    title VARCHAR(255) NOT NULL,
    subtitle VARCHAR(255),
    author TEXT NOT NULL,
    content TEXT NOT NULL,
    excerpt TEXT,
    tags TEXT[],
    status blogpost_status DEFAULT 'draft',
    slug VARCHAR(255) UNIQUE,
    views INT DEFAULT 0,
    comments_count INT DEFAULT 0
); 
    
