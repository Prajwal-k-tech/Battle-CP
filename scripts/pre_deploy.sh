#!/bin/bash
echo "🚀 Battle CP: Pre-Flight Check"
echo "=============================="

# 1. Check Backend Build
echo "Checking Backend Build..."
cd backend
if cargo check; then
    echo "✅ Backend compiles successfully."
else
    echo "❌ Backend build failed!"
    exit 1
fi
cd ..

# 2. Check Frontend Build
echo "Checking Frontend Build..."
cd frontend
if npm run build; then
    echo "✅ Frontend builds successfully."
else
    echo "❌ Frontend build failed!"
    exit 1
fi
cd ..

# 3. Check Dockerfile
if [ -f "backend/Dockerfile" ]; then
    echo "✅ Dockerfile exists."
else
    echo "❌ Dockerfile missing in backend/!"
    exit 1
fi

echo "=============================="
echo "✨ ALL SYSTEMS GO for Deployment!"
echo "Follow private_learning/deployment.md to ship it."
