# Registry API - Testing Guide

## Quick Test

### 1. Start the API

```bash
npm run dev
```

### 2. In another terminal, test health

```bash
curl http://localhost:3000/health
```

Expected: `{"status":"ok","timestamp":"..."}`

## Automated Tests

```bash
npm test
```

Runs Jest test suite covering:

- Authentication (signup, login, verify)
- Template management (publish, search, get)
- Reviews (post, get, update)
- Error handling

## Manual Testing with cURL

### Setup: Create User

```bash
TOKEN=$(curl -s -X POST http://localhost:3000/api/auth/signup \
  -H "Content-Type: application/json" \
  -d '{
    "email":"test@example.com",
    "username":"testuser",
    "password":"password123"
  }' | jq -r '.token')

echo "Token: $TOKEN"
```

### Search Templates

```bash
curl -X POST http://localhost:3000/api/templates/search \
  -H "Content-Type: application/json" \
  -d '{"query":"","limit":10}'
```

### Publish Template

Create test template:

```bash
mkdir -p /tmp/test-template/src
cat > /tmp/test-template/Cargo.toml << 'EOF'
[package]
name = "{{PROJECT_NAME}}"
version = "0.1.0"
edition = "2021"
EOF

echo '#![no_std]' > /tmp/test-template/src/lib.rs
echo '# Test Template' > /tmp/test-template/README.md

# Create ZIP
cd /tmp && zip -r test-template.zip test-template/

# Encode as base64
CONTENT=$(base64 -w 0 test-template.zip)

# Publish
curl -X POST http://localhost:3000/api/templates/publish \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d "{
    \"name\":\"test-template\",
    \"version\":\"1.0.0\",
    \"description\":\"Test template\",
    \"author\":\"Test User\",
    \"tags\":[\"test\"],
    \"content\":\"$CONTENT\"
  }"
```

### Get Template

```bash
curl http://localhost:3000/api/templates/test-template/1.0.0
```

### Post Review

```bash
# First, get the template ID from previous response
TEMPLATE_ID="<id-from-previous-response>"

curl -X POST http://localhost:3000/api/reviews/template/$TEMPLATE_ID/reviews \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{
    "rating":5,
    "comment":"Great template!"
  }'
```

### Get Reviews

```bash
curl http://localhost:3000/api/reviews/template/$TEMPLATE_ID
```

## Web UI Testing

1. Visit `http://localhost:3000`
2. Search for templates
3. Click login
4. Sign up with email/password
5. Try publishing (will fail without template, shows UI works)

## Docker Testing

### Start Services

```bash
docker-compose up
```

### Wait for MongoDB to be ready

```bash
docker-compose logs mongodb | grep "ready to accept connections"
```

### Test in another terminal

```bash
curl http://localhost:3000/health
```

### View Logs

```bash
docker-compose logs -f registry-api
```

### Stop

```bash
docker-compose down
```

## Load Testing

### Install Apache Bench

```bash
# macOS
brew install httpd

# Linux
sudo apt-get install apache2-utils

# Windows
choco install apache-httpd
```

### Run Load Test

```bash
# 100 concurrent requests, 1000 total
ab -n 1000 -c 100 http://localhost:3000/health

# Search endpoint
ab -n 500 -c 50 http://localhost:3000/api/templates/search \
  -p search-payload.json \
  -T "application/json"
```

Create `search-payload.json`:

```json
{ "query": "counter", "limit": 10 }
```

## Performance Benchmarks

**Expected Results:**

| Endpoint                              | Method | Time (ms) | Notes                |
| ------------------------------------- | ------ | --------- | -------------------- |
| /health                               | GET    | 10-20     | Always fast          |
| /api/templates/search                 | POST   | 50-200    | Depends on data size |
| /api/auth/login                       | POST   | 100-200   | Bcrypt verification  |
| /api/auth/signup                      | POST   | 150-300   | Password hashing     |
| /api/templates/publish                | POST   | 200-500   | File I/O             |
| /api/templates/:name:version/download | GET    | 50-100    | File serving         |

## Browser Testing

### Chrome DevTools

1. Open DevTools (F12)
2. Network tab to see requests
3. Storage > Local Storage to see JWT storage

### Firefox

1. Open Developer Tools (F12)
2. Storage > Local Storage
3. Console for JavaScript errors

### Safari

1. Show Develop menu: Safari > Preferences > Advanced > Show Develop menu
2. Develop > Web Inspector

## Edge Cases to Test

### Authentication

- [ ] Signup with existing email (should fail)
- [ ] Signup with weak password (should fail)
- [ ] Login with wrong password (should fail)
- [ ] Use expired token (should fail)
- [ ] Call publish without token (should fail)

### Templates

- [ ] Search with empty query (should return all)
- [ ] Search with no results (should return empty array)
- [ ] Publish without required fields (should fail)
- [ ] Download non-existent template (should 404)
- [ ] Install with bad version (should fail)

### Reviews

- [ ] Rate with score 0 (should fail)
- [ ] Rate with score 6 (should fail)
- [ ] Post review without auth (should fail)
- [ ] Update own review (should replace old)
- [ ] View reviews for non-existent template (should 404)

### Files

- [ ] Upload 100MB+ file (should fail with 413)
- [ ] Upload invalid ZIP (should fail)
- [ ] Upload empty template (should fail validation)

## Troubleshooting

### API won't start

```bash
# Check port
lsof -i :3000

# Check Node version
node --version  # Should be 18+

# Check dependencies
npm list
```

### Tests fail

```bash
# Clear cache
npm test -- --clearCache

# Run specific test
npm test -- api.test.ts

# Debug mode
node --inspect-brk node_modules/.bin/jest
```

### MongoDB connection error

```bash
# Check if MongoDB running
docker ps | grep mongodb

# Start MongoDB
docker run -d -p 27017:27017 mongo:6.0

# Check connection
mongosh localhost:27017
```

### CORS errors

```bash
# Check CORS_ORIGIN in .env
cat .env | grep CORS_ORIGIN

# Should match client origin in requests
```

## Continuous Testing

### Watch Mode

```bash
npm test -- --watch
```

### File Watcher for Dev

```bash
npm run dev
# Automatically reloads on changes
```

## Production Testing Checklist

Before deploying:

- [ ] All tests pass: `npm test`
- [ ] Code lints: `npm run lint`
- [ ] Builds successfully: `npm run build`
- [ ] Load test passes: 100+ concurrent users
- [ ] Security audit run
- [ ] Environment variables set correctly
- [ ] HTTPS certificate configured
- [ ] Rate limiting tested
- [ ] Error handling tested
- [ ] Database backups working
- [ ] Monitoring/logging configured
- [ ] Status page created

## Test Reports

After running tests, generate report:

```bash
npm test -- --coverage

# Output will show coverage %
# Aim for > 80% coverage
```

View HTML report:

```bash
open coverage/lcov-report/index.html
```

## Support

For test issues:

- Check console output
- Review test file: `src/tests/api.test.ts`
- Check Jest docs: https://jestjs.io
- Report issue: https://github.com/Nanle-code/StarForge/issues
