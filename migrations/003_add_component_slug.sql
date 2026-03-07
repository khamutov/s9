ALTER TABLE components ADD COLUMN slug TEXT;
CREATE UNIQUE INDEX idx_components_slug ON components(slug) WHERE slug IS NOT NULL;
