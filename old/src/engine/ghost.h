#ifndef ENGINE_GHOST_H
#define ENGINE_GHOST_H

#include <base/hash.h>
#include <engine/shared/protocol.h>

#include "kernel.h"

class CGhostInfo
{
public:
	char m_aOwner[MAX_NAME_LENGTH];
	char m_aMap[64];
	int m_NumTicks;
	int m_Time;
};

class IGhostRecorder : public IInterface
{
	MACRO_INTERFACE("ghostrecorder", 0)
public:
	virtual ~IGhostRecorder() {}

	virtual int Start(const char *pFilename, const char *pMap, SHA256_DIGEST MapSha256, const char *pName) = 0;
	virtual int Stop(int Ticks, int Time) = 0;

	virtual void WriteData(int Type, const void *pData, int Size) = 0;
	virtual bool IsRecording() const = 0;
};

class IGhostLoader : public IInterface
{
	MACRO_INTERFACE("ghostloader", 0)
public:
	virtual ~IGhostLoader() {}

	virtual int Load(const char *pFilename, const char *pMap, SHA256_DIGEST MapSha256, unsigned MapCrc) = 0;
	virtual void Close() = 0;

	virtual const CGhostInfo *GetInfo() const = 0;

	virtual bool ReadNextType(int *pType) = 0;
	virtual bool ReadData(int Type, void *pData, int Size) = 0;

	virtual bool GetGhostInfo(const char *pFilename, CGhostInfo *pInfo, const char *pMap, SHA256_DIGEST MapSha256, unsigned MapCrc) = 0;
};

#endif
