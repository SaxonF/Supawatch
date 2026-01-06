CREATE EXTENSION IF NOT EXISTS "pg_graphql" WITH SCHEMA "graphql" VERSION '1.5.11';
CREATE EXTENSION IF NOT EXISTS "pgcrypto" WITH SCHEMA "extensions" VERSION '1.3';
CREATE EXTENSION IF NOT EXISTS "pg_stat_statements" WITH SCHEMA "extensions" VERSION '1.11';
CREATE EXTENSION IF NOT EXISTS "supabase_vault" WITH SCHEMA "vault" VERSION '0.3.1';
CREATE EXTENSION IF NOT EXISTS "uuid-ossp" WITH SCHEMA "extensions" VERSION '1.1';
CREATE TYPE "status" AS ENUM ('todo', 'done');
CREATE OR REPLACE FUNCTION update_player_last_played() RETURNS trigger LANGUAGE plpgsql VOLATILE AS $$BEGIN
  UPDATE players SET last_played_at = NOW() WHERE id = NEW.player_id;
  RETURN NEW;
END;$$;
CREATE OR REPLACE FUNCTION generate_world(seed integer DEFAULT 0) RETURNS void LANGUAGE plpgsql VOLATILE AS $$DECLARE
  x_pos INTEGER;
  y_pos INTEGER;
  terrain_type TEXT;
  tile_desc TEXT;
  tile_features TEXT[];
  tile_resources TEXT[];
BEGIN
  -- Delete existing tiles for this seed
  DELETE FROM tiles WHERE world_seed = seed;

  -- Generate 10x10 world
  FOR y_pos IN 0..9 LOOP
    FOR x_pos IN 0..9 LOOP
      -- Determine terrain based on position (creates coherent biomes)

      -- Starting village at center
      IF x_pos = 5 AND y_pos = 5 THEN
        terrain_type := 'village';
        tile_desc := 'A small settlement of thatched cottages clusters around a central well. Smoke rises from chimneys, and the sounds of daily life fill the air.';
        tile_features := ARRAY['blacksmith', 'tavern', 'general store', 'well'];
        tile_resources := ARRAY['food', 'supplies', 'quests'];

      -- Second village
      ELSIF x_pos = 2 AND y_pos = 7 THEN
        terrain_type := 'village';
        tile_desc := 'A quiet farming village with fields of wheat stretching to the horizon. Friendly farmers wave as you pass.';
        tile_features := ARRAY['farm', 'barn', 'windmill'];
        tile_resources := ARRAY['food', 'wheat', 'gossip'];

      -- Corner ruins
      ELSIF (x_pos = 0 OR x_pos = 9) AND (y_pos = 0 OR y_pos = 9) THEN
        terrain_type := 'ruins';
        tile_desc := 'Crumbling stone walls hint at a structure once grand and imposing. Weathered carvings adorn the remaining archways.';
        tile_features := ARRAY['ancient statue', 'collapsed tunnel'];
        tile_resources := ARRAY['ancient artifacts', 'treasure'];

      -- River running through
      ELSIF (x_pos = 3 AND y_pos >= 2 AND y_pos <= 4) OR (x_pos >= 3 AND x_pos <= 6 AND y_pos = 3) THEN
        terrain_type := 'water';
        tile_desc := 'Crystal-clear water flows steadily, its current strong and cold. Fish dart beneath the surface.';
        tile_features := ARRAY['fishing spot', 'smooth stones'];
        tile_resources := ARRAY['fish', 'water plants'];

      -- Mountain range (northeast)
      ELSIF y_pos >= 7 AND x_pos >= 6 THEN
        terrain_type := 'mountains';
        tile_desc := 'Rocky crags rise sharply, their peaks disappearing into the clouds. Cold winds whip around the exposed rock.';
        tile_features := ARRAY['cave entrance', 'mineral vein'];
        tile_resources := ARRAY['iron ore', 'stone', 'gems'];

      -- Swamp (southwest corner)
      ELSIF x_pos <= 2 AND y_pos <= 2 THEN
        terrain_type := 'swamp';
        tile_desc := 'Murky water pools between twisted, moss-covered trees. The air is thick with moisture and the buzz of insects.';
        tile_features := ARRAY['old bridge', 'giant lily pad'];
        tile_resources := ARRAY['peat', 'rare herbs'];

      -- Desert (east side, low y)
      ELSIF x_pos >= 7 AND y_pos <= 3 THEN
        terrain_type := 'desert';
        tile_desc := 'Endless dunes of golden sand ripple toward the horizon. The scorching heat shimmers above the barren ground.';
        tile_features := ARRAY['cactus grove', 'ancient bones'];
        tile_resources := ARRAY['sand', 'cactus fruit', 'salt'];

      -- Forest and plains mixed elsewhere
      ELSIF (x_pos + y_pos) % 3 = 0 THEN
        terrain_type := 'forest';
        tile_desc := 'Dense trees tower overhead, their canopy filtering the sunlight into dappled patterns. Birdsong echoes through the woodland.';
        tile_features := ARRAY['old oak tree', 'berry bushes', 'mushroom circle'];
        tile_resources := ARRAY['wood', 'herbs', 'berries'];

      ELSE
        terrain_type := 'plains';
        tile_desc := 'Rolling grasslands stretch toward the horizon, swaying in the gentle breeze. Wildflowers dot the landscape.';
        tile_features := ARRAY['wildflower patch', 'rabbit burrow'];
        tile_resources := ARRAY['wheat', 'flax', 'stone'];
      END IF;

      INSERT INTO tiles (x, y, terrain, description, features, resources, world_seed, discovered)
      VALUES (x_pos, y_pos, terrain_type, tile_desc, tile_features, tile_resources, seed,
        -- Starting area is discovered
        (ABS(x_pos - 5) <= 1 AND ABS(y_pos - 5) <= 1)
      );
    END LOOP;
  END LOOP;
END;$$;
CREATE TABLE "players" (
  "agility" integer NOT NULL DEFAULT 10,
  "created_at" timestamp with time zone NOT NULL DEFAULT now(),
  "experience" integer NOT NULL DEFAULT 0,
  "health" integer NOT NULL DEFAULT 100,
  "id" uuid DEFAULT gen_random_uuid(),
  "intelligence" integer NOT NULL DEFAULT 10,
  "last_played_at" timestamp with time zone NOT NULL DEFAULT now(),
  "level" integer NOT NULL DEFAULT 1,
  "luck" integer NOT NULL DEFAULT 10,
  "max_health" integer NOT NULL DEFAULT 100,
  "max_stamina" integer NOT NULL DEFAULT 100,
  "money" integer NOT NULL DEFAULT 50,
  "name" text NOT NULL,
  "position_x" integer NOT NULL DEFAULT 5,
  "position_y" integer NOT NULL DEFAULT 5,
  "stamina" integer NOT NULL DEFAULT 100,
  "strength" integer NOT NULL DEFAULT 10,
  PRIMARY KEY ("id")
);
CREATE TABLE "inventory_items" (
  "created_at" timestamp with time zone NOT NULL DEFAULT now(),
  "description" text NOT NULL,
  "id" uuid DEFAULT gen_random_uuid(),
  "item_type" text NOT NULL,
  "name" text NOT NULL,
  "player_id" uuid NOT NULL,
  "quantity" integer NOT NULL DEFAULT 1,
  PRIMARY KEY ("id"),
  CONSTRAINT "inventory_items_item_type_check" CHECK ((item_type = ANY (ARRAY['weapon'::text, 'armor'::text, 'consumable'::text, 'material'::text, 'tool'::text, 'misc'::text])))
);
CREATE TABLE "tiles" (
  "building_built_at" timestamp with time zone,
  "building_built_by" uuid,
  "building_description" text,
  "building_name" text,
  "building_type" text,
  "description" text NOT NULL,
  "discovered" boolean NOT NULL DEFAULT false,
  "features" text[] NOT NULL DEFAULT '{}'::text[],
  "id" uuid DEFAULT gen_random_uuid(),
  "owner_id" uuid,
  "resources" text[] NOT NULL DEFAULT '{}'::text[],
  "status" text,
  "terrain" text NOT NULL,
  "world_seed" integer NOT NULL DEFAULT 0,
  "x" integer NOT NULL,
  "y" integer NOT NULL,
  PRIMARY KEY ("id"),
  CONSTRAINT "tiles_terrain_check" CHECK ((terrain = ANY (ARRAY['forest'::text, 'plains'::text, 'mountains'::text, 'water'::text, 'desert'::text, 'swamp'::text, 'village'::text, 'ruins'::text])))
);
CREATE UNIQUE INDEX "tiles_status_key" ON "tiles" ("status");
ALTER TABLE "tiles" ENABLE ROW LEVEL SECURITY;
CREATE TABLE "game_sessions" (
  "id" uuid DEFAULT gen_random_uuid(),
  "last_activity_at" timestamp with time zone NOT NULL DEFAULT now(),
  "player_id" uuid NOT NULL,
  "started_at" timestamp with time zone NOT NULL DEFAULT now(),
  "world_seed" integer NOT NULL,
  PRIMARY KEY ("id")
);
CREATE TABLE "message_history" (
  "content" text NOT NULL,
  "created_at" timestamp with time zone NOT NULL DEFAULT now(),
  "id" uuid DEFAULT gen_random_uuid(),
  "player_id" uuid NOT NULL,
  "role" text NOT NULL,
  PRIMARY KEY ("id"),
  CONSTRAINT "message_history_role_check" CHECK ((role = ANY (ARRAY['user'::text, 'assistant'::text, 'system'::text])))
);
CREATE POLICY "Enable insert for authenticated users only" ON "tiles" FOR ALL TO authenticated USING (true) WITH CHECK (true);
CREATE TRIGGER "on_message_update_player" AFTER INSERT ON "message_history" FOR EACH ROW EXECUTE FUNCTION update_player_last_played();
ALTER TABLE "inventory_items" ADD CONSTRAINT "inventory_items_player_id_fkey" FOREIGN KEY ("player_id") REFERENCES "players"("id") ON DELETE CASCADE;
ALTER TABLE "tiles" ADD CONSTRAINT "tiles_owner_id_fkey" FOREIGN KEY ("owner_id") REFERENCES "players"("id") ON DELETE SET NULL;
ALTER TABLE "tiles" ADD CONSTRAINT "tiles_building_built_by_fkey" FOREIGN KEY ("building_built_by") REFERENCES "players"("id") ON DELETE SET NULL;
ALTER TABLE "game_sessions" ADD CONSTRAINT "game_sessions_player_id_fkey" FOREIGN KEY ("player_id") REFERENCES "players"("id") ON DELETE CASCADE;
ALTER TABLE "message_history" ADD CONSTRAINT "message_history_player_id_fkey" FOREIGN KEY ("player_id") REFERENCES "players"("id") ON DELETE CASCADE;